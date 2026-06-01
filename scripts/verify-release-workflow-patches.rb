#!/usr/bin/env ruby
# Verify hand-maintained release workflow structure survived dist generate.
# frozen_string_literal: true

require "yaml"

ROOT = File.expand_path("..", __dir__)
WORKFLOW = File.join(ROOT, ".github/workflows/release.yml")
CI_WORKFLOW = File.join(ROOT, ".github/workflows/ci.yml")
RELEASE_SCRIPT = File.join(ROOT, "scripts/create-github-release.sh")

def fail!(message)
  warn message
  exit 1
end

unless File.file?(WORKFLOW)
  fail!("missing workflow: #{WORKFLOW}")
end

unless File.file?(RELEASE_SCRIPT)
  fail!("missing release script: #{RELEASE_SCRIPT}")
end

unless File.file?(CI_WORKFLOW)
  fail!("missing workflow: #{CI_WORKFLOW}")
end

workflow = YAML.load_file(WORKFLOW)
jobs = workflow.fetch("jobs")
workflow_text = File.read(WORKFLOW)
ci_workflow = YAML.load_file(CI_WORKFLOW)
ci_jobs = ci_workflow.fetch("jobs")
ci_text = File.read(CI_WORKFLOW)

errors = []

if workflow_text.match?(/^\s*pull_request:/m)
  errors << "release workflow must not use pull_request trigger (CI dispatches via workflow_dispatch)"
end
unless workflow_text.match?(/^\s*workflow_dispatch:/m)
  errors << "release workflow missing workflow_dispatch trigger"
end

trigger_release = ci_jobs["trigger-release"] or errors << "ci.yml missing trigger-release job"
if trigger_release
  unless ci_text.include?("createWorkflowDispatch")
    errors << "ci.yml trigger-release must dispatch Release via createWorkflowDispatch"
  end
  unless ci_text.include?("workflow_id: 'release.yml'") || ci_text.include?('workflow_id: "release.yml"')
    errors << "ci.yml trigger-release must dispatch release.yml"
  end
  unless ci_text.include?("pr_number")
    errors << "ci.yml trigger-release must pass pr_number input"
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

plan = jobs["plan"] or errors << "missing plan job"
if plan
  outputs = plan["outputs"] || {}
  errors << "plan job missing release_version output" unless outputs.key?("release_version")
  errors << "plan job missing release-version.sh step" unless step_runs?(plan["steps"], "release-version.sh")
  errors << "plan job missing require-ci-success-on-commit.sh step" unless step_runs?(plan["steps"], "require-ci-success-on-commit.sh")
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

postbuild = jobs["custom-release-postbuild"] or errors << "missing custom-release-postbuild job"
if postbuild
  inputs = postbuild.dig("with") || {}
  unless inputs.key?("release_version")
    errors << "custom-release-postbuild missing release_version input"
  end
  unless postbuild.dig("uses").to_s.include?("release-postbuild.yml")
    errors << "custom-release-postbuild must call release-postbuild.yml"
  end
end

host = jobs["host"] or errors << "missing host job"
if host
  steps = host["steps"] || []
  errors << "host missing plan manifest download" unless step_downloads?(steps, "artifacts-plan-dist-manifest")
  errors << "host missing build manifest download" unless step_downloads?(steps, "artifacts-dist-manifests")
  unless step_runs?(steps, "create-github-release.sh")
    errors << "host missing create-github-release.sh step"
  end
  bundle_download = steps.any? do |step|
    next false unless step.is_a?(Hash) && step["uses"].to_s.include?("download-artifact")

    name = step.dig("with", "name").to_s
    name.start_with?("enc-sync-") && name.include?("release_version")
  end
  errors << "host missing enc-sync release bundle download" unless bundle_download
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
