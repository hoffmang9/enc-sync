#!/usr/bin/env ruby
# Verify hand-maintained release workflow invariants survived dist generate.
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

REQUIRED_FILES = [WORKFLOW, POSTBUILD_WORKFLOW, CI_WORKFLOW, RELEASE_SCRIPT, CLEANUP_SCRIPT].freeze

class Workflow
  attr_reader :path, :data, :text

  def initialize(path)
    @path = path
    @data = YAML.load_file(path)
    @text = File.read(path)
  end

  def jobs
    data.fetch("jobs")
  end

  def job(name)
    jobs[name]
  end

  def permissions
    data["permissions"] || {}
  end

  def trigger?(name)
    text.match?(/^\s*#{Regexp.escape(name)}:/)
  end
end

def fail!(errors)
  errors.each { |err| warn "release workflow check failed: #{err}" }
  warn "re-apply patches listed in dist-workspace.toml"
  exit 1
end

def needs?(job, dependency)
  needs = job&.fetch("needs", nil)
  Array(needs).include?(dependency) || needs == dependency
end

def permission(job_or_permissions, key)
  permissions = job_or_permissions&.fetch("permissions", nil) || job_or_permissions
  permissions.is_a?(Hash) ? permissions[key] : nil
end

def steps(job)
  Array(job&.fetch("steps", nil))
end

def step_runs?(job, substring)
  steps(job).any? { |step| step.is_a?(Hash) && step["run"].to_s.include?(substring) }
end

def step_contains?(job, substring)
  steps(job).any? do |step|
    next false unless step.is_a?(Hash)

    step["run"].to_s.include?(substring) ||
      step.dig("with", "script").to_s.include?(substring)
  end
end

def step_uploads?(job, artifact_name)
  steps(job).any? do |step|
    step.is_a?(Hash) &&
      step["uses"].to_s.include?("upload-artifact") &&
      step.dig("with", "name") == artifact_name
  end
end

def step_downloads?(job, artifact_name)
  steps(job).any? do |step|
    step.is_a?(Hash) &&
      step["uses"].to_s.include?("download-artifact") &&
      step.dig("with", "name") == artifact_name
  end
end

def step_downloads_pattern?(job, pattern)
  steps(job).any? do |step|
    step.is_a?(Hash) &&
      step["uses"].to_s.include?("download-artifact") &&
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

def check_job(errors, workflow, name)
  job = workflow.job(name)
  errors << "#{workflow.path} missing #{name} job" unless job
  job
end

errors = []
REQUIRED_FILES.each { |path| errors << "missing file: #{path}" unless File.file?(path) }
fail!(errors) unless errors.empty?

release = Workflow.new(WORKFLOW)
postbuild = Workflow.new(POSTBUILD_WORKFLOW)
ci = Workflow.new(CI_WORKFLOW)

errors << "release workflow must not use pull_request trigger" if release.trigger?("pull_request")
errors << "release workflow missing workflow_dispatch trigger" unless release.trigger?("workflow_dispatch")
errors << "release workflow missing statuses: write permission" unless permission(release.permissions, "statuses") == "write"
errors << "release workflow should keep workflow-level actions permission to read" unless permission(release.permissions, "actions") == "read"

trigger_release = check_job(errors, ci, "trigger-release")
if trigger_release
  errors << "ci trigger-release must need lint" unless needs?(trigger_release, "lint")
  errors << "ci trigger-release must not wait for full CI" if needs?(trigger_release, "ci")
  [
    "createWorkflowDispatch",
    "workflow_id: 'release.yml'",
    "pr_number",
    "release/enc-sync"
  ].each do |needle|
    errors << "ci trigger-release missing #{needle}" unless step_contains?(trigger_release, needle)
  end
end

plan = check_job(errors, release, "plan")
if plan
  outputs = plan["outputs"] || {}
  errors << "plan job missing release_version output" unless outputs.key?("release_version")
  [
    "release-version.sh",
    "require-ci-success-on-commit.sh",
    "report-pr-release-status.sh pending"
  ].each do |script|
    errors << "plan job missing #{script}" unless step_runs?(plan, script)
  end
end

build_global = check_job(errors, release, "build-global-artifacts")
if build_global
  errors << "build-global-artifacts must be gated on publishing" unless build_global["if"].to_s.include?("publishing")
  errors << "build-global-artifacts missing dist manifests upload" unless step_uploads?(build_global, "artifacts-dist-manifests")
end

errors << "require-ci-success must live in release-postbuild" if release.job("require-ci-success")
%w[announce custom-package-macos-universal custom-finalize-release-artifacts].each do |removed_job|
  errors << "#{removed_job} job should not be present" if release.job(removed_job)
end

postbuild_call = check_job(errors, release, "custom-release-postbuild")
if postbuild_call
  inputs = postbuild_call["with"] || {}
  errors << "custom-release-postbuild missing release_version input" unless inputs.key?("release_version")
  errors << "custom-release-postbuild missing publishing input" unless inputs.key?("publishing")
  errors << "custom-release-postbuild must call release-postbuild.yml" unless postbuild_call["uses"].to_s.include?("release-postbuild.yml")
  errors << "custom-release-postbuild must not wait for CI before macOS universal packaging" if needs?(postbuild_call, "require-ci-success")
  errors << "custom-release-postbuild must grant actions: write" unless permission(postbuild_call, "actions") == "write"
  errors << "custom-release-postbuild should grant only contents: read" unless permission(postbuild_call, "contents") == "read"
end

package_macos = check_job(errors, postbuild, "package-macos-universal")
if package_macos
  errors << "package-macos-universal should grant actions: read" unless permission(package_macos, "actions") == "read"
  errors << "package-macos-universal should grant contents: read" unless permission(package_macos, "contents") == "read"
end

postbuild_require_ci = check_job(errors, postbuild, "require-ci-success")
if postbuild_require_ci
  errors << "postbuild require-ci-success must run only for PR builds" unless postbuild_require_ci["if"].to_s.include?("inputs.publishing")
  errors << "postbuild require-ci-success must wait for package-macos-universal" unless needs?(postbuild_require_ci, "package-macos-universal")
  errors << "postbuild require-ci-success must run require-ci-success-on-commit.sh" unless step_runs?(postbuild_require_ci, "require-ci-success-on-commit.sh")
  errors << "postbuild require-ci-success must set WAIT_FOR_CI_SECONDS" if postbuild_require_ci.dig("env", "WAIT_FOR_CI_SECONDS").to_s.empty?
  errors << "postbuild require-ci-success should grant actions: read" unless permission(postbuild_require_ci, "actions") == "read"
  errors << "postbuild require-ci-success should grant contents: read" unless permission(postbuild_require_ci, "contents") == "read"
end

finalize = check_job(errors, postbuild, "finalize-release-artifacts")
if finalize
  errors << "finalize must wait for package-macos-universal" unless needs?(finalize, "package-macos-universal")
  errors << "finalize must wait for require-ci-success" unless needs?(finalize, "require-ci-success")
  errors << "finalize if condition must require CI success on PR releases" unless finalize["if"].to_s.include?("require-ci-success")
  errors << "finalize must download linux artifacts separately" unless step_downloads_pattern?(finalize, "artifacts-build-local-*-linux-musl")
  errors << "finalize must download windows artifacts separately" unless step_downloads_pattern?(finalize, "artifacts-build-local-*windows*")
  errors << "finalize must not download all local artifacts" if step_downloads_pattern?(finalize, "artifacts-build-local-*")
  errors << "finalize must clean up PR workflow artifacts" unless step_runs?(finalize, "cleanup-workflow-artifacts.sh")
  errors << "finalize cleanup must be PR-only" unless postbuild.text.include?("if: inputs.publishing != 'true'")
  errors << "finalize must grant actions: write" unless permission(finalize, "actions") == "write"
  errors << "finalize should grant contents: read" unless permission(finalize, "contents") == "read"
end

cleanup = check_job(errors, release, "cleanup-workflow-artifacts")
if cleanup
  errors << "cleanup job must run cleanup-workflow-artifacts.sh" unless step_runs?(cleanup, "cleanup-workflow-artifacts.sh")
  errors << "cleanup job must be gated on publishing" unless cleanup["if"].to_s.include?("publishing")
  errors << "cleanup job must delete all tag-release artifacts" unless cleanup.dig("env", "DELETE_ALL").to_s == "true"
  errors << "cleanup job must need host" unless needs?(cleanup, "host")
  errors << "cleanup job must grant actions: write" unless permission(cleanup, "actions") == "write"
  errors << "cleanup job should grant contents: read" unless permission(cleanup, "contents") == "read"
end

report_status = check_job(errors, release, "report-pr-release-status")
if report_status
  errors << "report-pr-release-status must run report-pr-release-status.sh report" unless step_runs?(report_status, "report-pr-release-status.sh report")
end

host = check_job(errors, release, "host")
if host
  errors << "host missing plan manifest download" unless step_downloads?(host, "artifacts-plan-dist-manifest")
  errors << "host missing build manifest download" unless step_downloads?(host, "artifacts-dist-manifests")
  errors << "host missing release bundle download" unless step_downloads?(host, BUNDLE_ARTIFACT_NAME)
  errors << "host missing create-github-release.sh" unless step_runs?(host, "create-github-release.sh")
  errors << "host must need custom-release-postbuild" unless needs?(host, "custom-release-postbuild")
end

release_script = File.read(RELEASE_SCRIPT)
errors << "create-github-release.sh must not use dist host --steps=upload" if release_script.include?("--steps=upload")
errors << "create-github-release.sh must run dist host --steps=release" unless release_script.include?("--steps=release")

keep_success, keep_ids, keep_stderr = cleanup_ids_for_fixture(
  CLEANUP_SCRIPT,
  { "KEEP_ARTIFACT_NAME" => "enc-sync-0.1.0-pr.5.abcdef0" }
)
unless keep_success && keep_ids == %w[101 103]
  errors << "cleanup keep-one mode selected #{keep_ids.inspect} (stderr: #{keep_stderr.strip})"
end

delete_all_success, delete_all_ids, delete_all_stderr = cleanup_ids_for_fixture(
  CLEANUP_SCRIPT,
  { "DELETE_ALL" => "true" }
)
unless delete_all_success && delete_all_ids == %w[101 102 103]
  errors << "cleanup DELETE_ALL mode selected #{delete_all_ids.inspect} (stderr: #{delete_all_stderr.strip})"
end

if errors.empty?
  puts "release workflow custom patches present"
else
  fail!(errors)
end
