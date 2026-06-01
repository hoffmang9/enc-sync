#!/usr/bin/env ruby
# Verify hand-maintained release workflow structure survived dist generate.
# frozen_string_literal: true

require "json"
require "open3"
require "tempfile"
require "yaml"

ROOT = File.expand_path("..", __dir__)
WORKFLOW = File.join(ROOT, ".github/workflows/release.yml")
POSTBUILD_WORKFLOW = File.join(ROOT, ".github/workflows/release-postbuild.yml")
CI_WORKFLOW = File.join(ROOT, ".github/workflows/ci.yml")
RELEASE_SCRIPT = File.join(ROOT, "scripts/create-github-release.sh")
CLEANUP_SCRIPT = File.join(ROOT, "scripts/cleanup-workflow-artifacts.sh")
BUNDLE_ARTIFACT_NAME = "enc-sync-${{ needs.plan.outputs.release_version }}"

def fail!(message)
  warn message
  exit 1
end

unless File.file?(WORKFLOW)
  fail!("missing workflow: #{WORKFLOW}")
end

unless File.file?(POSTBUILD_WORKFLOW)
  fail!("missing workflow: #{POSTBUILD_WORKFLOW}")
end

unless File.file?(RELEASE_SCRIPT)
  fail!("missing release script: #{RELEASE_SCRIPT}")
end

unless File.file?(CLEANUP_SCRIPT)
  fail!("missing cleanup script: #{CLEANUP_SCRIPT}")
end

unless File.file?(CI_WORKFLOW)
  fail!("missing workflow: #{CI_WORKFLOW}")
end

workflow = YAML.load_file(WORKFLOW)
jobs = workflow.fetch("jobs")
workflow_text = File.read(WORKFLOW)
postbuild_workflow = YAML.load_file(POSTBUILD_WORKFLOW)
postbuild_text = File.read(POSTBUILD_WORKFLOW)
postbuild_jobs = postbuild_workflow.fetch("jobs")
ci_workflow = YAML.load_file(CI_WORKFLOW)
ci_jobs = ci_workflow.fetch("jobs")
ci_text = File.read(CI_WORKFLOW)

def permission_value(permissions, key)
  return nil unless permissions.is_a?(Hash)

  permissions[key]
end

errors = []
workflow_permissions = workflow["permissions"] || {}

if workflow_text.match?(/^\s*pull_request:/m)
  errors << "release workflow must not use pull_request trigger (CI dispatches via workflow_dispatch)"
end
unless workflow_text.match?(/^\s*workflow_dispatch:/m)
  errors << "release workflow missing workflow_dispatch trigger"
end
unless workflow_text.include?("statuses: write")
  errors << "release workflow missing statuses: write permission"
end
unless permission_value(workflow_permissions, "actions") == "read"
  errors << "release workflow should keep workflow-level actions permission to read"
end
if permission_value(workflow_permissions, "actions") == "write"
  errors << "release workflow must not grant workflow-level actions: write"
end

trigger_release = ci_jobs["trigger-release"] or errors << "ci.yml missing trigger-release job"
if trigger_release
  unless Array(trigger_release["needs"]).include?("lint") || trigger_release["needs"] == "lint"
    errors << "ci.yml trigger-release must need lint so release builds start after lint"
  end
  if Array(trigger_release["needs"]).include?("ci") || trigger_release["needs"] == "ci"
    errors << "ci.yml trigger-release must not wait for full CI"
  end
  unless ci_text.include?("createWorkflowDispatch")
    errors << "ci.yml trigger-release must dispatch Release via createWorkflowDispatch"
  end
  unless ci_text.include?("workflow_id: 'release.yml'") || ci_text.include?('workflow_id: "release.yml"')
    errors << "ci.yml trigger-release must dispatch release.yml"
  end
  unless ci_text.include?("pr_number")
    errors << "ci.yml trigger-release must pass pr_number input"
  end
  unless ci_text.include?("release/enc-sync")
    errors << "ci.yml trigger-release must post release/enc-sync commit status"
  end
end

def step_runs?(steps, substring)
  Array(steps).any? { |step| step.is_a?(Hash) && step["run"].to_s.include?(substring) }
end

def step_uploads?(steps, artifact_name)
  Array(steps).any? do |step|
    next false unless step.is_a?(Hash) && step["uses"].to_s.include?("upload-artifact")

    step.dig("with", "name") == artifact_name
  end
end

def step_downloads?(steps, artifact_name)
  Array(steps).any? do |step|
    next false unless step.is_a?(Hash) && step["uses"].to_s.include?("download-artifact")

    step.dig("with", "name") == artifact_name
  end
end

def step_download_pattern?(steps, pattern)
  Array(steps).any? do |step|
    next false unless step.is_a?(Hash) && step["uses"].to_s.include?("download-artifact")

    step.dig("with", "pattern") == pattern
  end
end

def cleanup_ids_for_fixture(script, env)
  fixture = {
    artifacts: [
      { id: 101, name: "artifacts-build-local-x86_64-unknown-linux-musl" },
      { id: 102, name: "enc-sync-0.1.0-pr.5.abcdef0" },
      { id: 103, name: "artifacts-plan-dist-manifest" }
    ]
  }

  Tempfile.create(["workflow-artifacts", ".json"]) do |file|
    file.write(JSON.generate(fixture))
    file.close

    stdout, stderr, status = Open3.capture3(
      {
        "ARTIFACTS_JSON" => file.path,
        "GITHUB_REPOSITORY" => "hoffmang9/enc-sync",
        "GITHUB_RUN_ID" => "123"
      }.merge(env),
      script,
      "--print-delete-ids"
    )

    [status.success?, stdout.lines.map(&:strip).reject(&:empty?), stderr]
  end
end

plan = jobs["plan"] or errors << "missing plan job"
if plan
  outputs = plan["outputs"] || {}
  errors << "plan job missing release_version output" unless outputs.key?("release_version")
  errors << "plan job missing release-version.sh step" unless step_runs?(plan["steps"], "release-version.sh")
  errors << "plan job missing require-ci-success-on-commit.sh step" unless step_runs?(plan["steps"], "require-ci-success-on-commit.sh")
  errors << "plan job missing report-pr-release-status.sh pending step" unless step_runs?(plan["steps"], "report-pr-release-status.sh pending")
end

build_global = jobs["build-global-artifacts"] or errors << "missing build-global-artifacts job"
if build_global
  unless build_global["if"].to_s.include?("publishing")
    errors << "build-global-artifacts must be gated on publishing"
  end
  unless step_uploads?(build_global["steps"], "artifacts-dist-manifests")
    errors << "build-global-artifacts missing artifacts-dist-manifests upload"
  end
end

if jobs.key?("require-ci-success")
  errors << "require-ci-success must live in release-postbuild after macOS universal packaging"
end

postbuild = jobs["custom-release-postbuild"] or errors << "missing custom-release-postbuild job"
if postbuild
  inputs = postbuild.dig("with") || {}
  unless inputs.key?("release_version")
    errors << "custom-release-postbuild missing release_version input"
  end
  unless inputs.key?("publishing")
    errors << "custom-release-postbuild missing publishing input"
  end
  unless postbuild.dig("uses").to_s.include?("release-postbuild.yml")
    errors << "custom-release-postbuild must call release-postbuild.yml"
  end
  needs = Array(postbuild["needs"])
  if needs.include?("require-ci-success")
    errors << "custom-release-postbuild must not wait for CI before macOS universal packaging"
  end
  if postbuild["if"].to_s.include?("require-ci-success")
    errors << "custom-release-postbuild if condition must not gate before macOS universal packaging"
  end
  unless permission_value(postbuild["permissions"], "actions") == "write"
    errors << "custom-release-postbuild must grant actions: write for PR artifact cleanup"
  end
  unless permission_value(postbuild["permissions"], "contents") == "read"
    errors << "custom-release-postbuild should grant only contents: read"
  end
end

package_macos = postbuild_jobs["package-macos-universal"] or errors << "release-postbuild missing package-macos-universal job"
if package_macos
  unless permission_value(package_macos["permissions"], "actions") == "read"
    errors << "package-macos-universal should grant actions: read"
  end
  unless permission_value(package_macos["permissions"], "contents") == "read"
    errors << "package-macos-universal should grant contents: read"
  end
end

postbuild_require_ci = postbuild_jobs["require-ci-success"] or errors << "release-postbuild missing require-ci-success job"
if postbuild_require_ci
  unless postbuild_require_ci["if"].to_s.include?("inputs.publishing")
    errors << "release-postbuild require-ci-success must run only for PR builds"
  end
  unless Array(postbuild_require_ci["needs"]).include?("package-macos-universal") || postbuild_require_ci["needs"] == "package-macos-universal"
    errors << "release-postbuild require-ci-success must wait for package-macos-universal"
  end
  unless step_runs?(postbuild_require_ci["steps"], "require-ci-success-on-commit.sh")
    errors << "release-postbuild require-ci-success must run require-ci-success-on-commit.sh"
  end
  unless postbuild_require_ci.dig("env", "WAIT_FOR_CI_SECONDS").to_s != ""
    errors << "release-postbuild require-ci-success must wait for CI completion"
  end
  unless permission_value(postbuild_require_ci["permissions"], "actions") == "read"
    errors << "release-postbuild require-ci-success should grant actions: read"
  end
  unless permission_value(postbuild_require_ci["permissions"], "contents") == "read"
    errors << "release-postbuild require-ci-success should grant contents: read"
  end
end

finalize = postbuild_jobs["finalize-release-artifacts"] or errors << "release-postbuild missing finalize-release-artifacts job"
if finalize
  finalize_steps = finalize["steps"] || []
  finalize_needs = Array(finalize["needs"])
  unless finalize_needs.include?("package-macos-universal")
    errors << "finalize must wait for package-macos-universal"
  end
  unless finalize_needs.include?("require-ci-success")
    errors << "finalize must wait for require-ci-success before bundling PR artifacts"
  end
  unless finalize["if"].to_s.include?("require-ci-success")
    errors << "finalize if condition must require CI success on PR releases"
  end
  unless step_download_pattern?(finalize_steps, "artifacts-build-local-*-linux-musl")
    errors << "finalize must download linux artifacts without mac per-arch builds"
  end
  unless step_download_pattern?(finalize_steps, "artifacts-build-local-*windows*")
    errors << "finalize must download windows artifacts separately"
  end
  if step_download_pattern?(finalize_steps, "artifacts-build-local-*")
    errors << "finalize must not download all artifacts-build-local-* (includes mac per-arch)"
  end
  unless step_runs?(finalize_steps, "cleanup-workflow-artifacts.sh")
    errors << "finalize must run cleanup-workflow-artifacts.sh for PR artifact cleanup"
  end
  unless postbuild_text.include?("if: inputs.publishing != 'true'")
    errors << "finalize cleanup must be gated on non-publishing (PR) runs"
  end
  unless permission_value(finalize["permissions"], "actions") == "write"
    errors << "finalize must grant actions: write for PR artifact cleanup"
  end
  unless permission_value(finalize["permissions"], "contents") == "read"
    errors << "finalize should grant contents: read"
  end
end

cleanup = jobs["cleanup-workflow-artifacts"] or errors << "missing cleanup-workflow-artifacts job"
if cleanup
  unless step_runs?(cleanup["steps"], "cleanup-workflow-artifacts.sh")
    errors << "cleanup-workflow-artifacts must run cleanup-workflow-artifacts.sh"
  end
  unless cleanup["if"].to_s.include?("publishing")
    errors << "cleanup-workflow-artifacts must be gated on publishing (tag releases)"
  end
  unless cleanup.dig("env", "DELETE_ALL") == true || cleanup.dig("env", "DELETE_ALL") == "true"
    errors << "cleanup-workflow-artifacts must set DELETE_ALL for tag releases"
  end
  needs = Array(cleanup["needs"])
  errors << "cleanup-workflow-artifacts must need host" unless needs.include?("host")
  unless permission_value(cleanup["permissions"], "actions") == "write"
    errors << "cleanup-workflow-artifacts must grant actions: write for tag artifact cleanup"
  end
  unless permission_value(cleanup["permissions"], "contents") == "read"
    errors << "cleanup-workflow-artifacts should grant contents: read"
  end
end

keep_success, keep_ids, keep_stderr = cleanup_ids_for_fixture(
  CLEANUP_SCRIPT,
  { "KEEP_ARTIFACT_NAME" => "enc-sync-0.1.0-pr.5.abcdef0" }
)
unless keep_success && keep_ids == %w[101 103]
  errors << "cleanup-workflow-artifacts keep-one mode selected #{keep_ids.inspect} (stderr: #{keep_stderr.strip})"
end

delete_all_success, delete_all_ids, delete_all_stderr = cleanup_ids_for_fixture(
  CLEANUP_SCRIPT,
  { "DELETE_ALL" => "true" }
)
unless delete_all_success && delete_all_ids == %w[101 102 103]
  errors << "cleanup-workflow-artifacts DELETE_ALL mode selected #{delete_all_ids.inspect} (stderr: #{delete_all_stderr.strip})"
end

report_status = jobs["report-pr-release-status"] or errors << "missing report-pr-release-status job"
if report_status && !step_runs?(report_status["steps"], "report-pr-release-status.sh report")
  errors << "report-pr-release-status must run report-pr-release-status.sh report"
end

host = jobs["host"] or errors << "missing host job"
if host
  steps = host["steps"] || []
  errors << "host missing plan manifest download" unless step_downloads?(steps, "artifacts-plan-dist-manifest")
  errors << "host missing build manifest download" unless step_downloads?(steps, "artifacts-dist-manifests")
  unless step_runs?(steps, "create-github-release.sh")
    errors << "host missing create-github-release.sh step"
  end
  errors << "host missing enc-sync release bundle download" unless step_downloads?(steps, BUNDLE_ARTIFACT_NAME)
  needs = Array(host["needs"])
  unless needs.include?("custom-release-postbuild")
    errors << "host must need custom-release-postbuild"
  end
end

errors << "announce job should be omitted (release is created in host)" if jobs.key?("announce")
errors << "custom-package-macos-universal job should be removed (use custom-release-postbuild)" if jobs.key?("custom-package-macos-universal")
errors << "custom-finalize-release-artifacts job should be removed (use custom-release-postbuild)" if jobs.key?("custom-finalize-release-artifacts")

release_script = File.read(RELEASE_SCRIPT)
if release_script.include?("--steps=upload")
  errors << "create-github-release.sh must not use dist host --steps=upload"
end
unless release_script.include?("--steps=release")
  errors << "create-github-release.sh must run dist host --steps=release"
end

if errors.empty?
  puts "release workflow custom patches present"
else
  errors.each { |err| warn "release workflow check failed: #{err}" }
  warn "re-apply patches listed in dist-workspace.toml"
  exit 1
end
