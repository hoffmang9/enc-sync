#!/usr/bin/env ruby
# Verify hand-maintained release workflow structure survived dist generate.
# Uses YAML parsing (Ruby Psych) rather than string greps.
# frozen_string_literal: true

require "yaml"

ROOT = File.expand_path("..", __dir__)
WORKFLOW = File.join(ROOT, ".github/workflows/release.yml")
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

workflow = YAML.load_file(WORKFLOW)
jobs = workflow.fetch("jobs")

errors = []

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
end

build_global = jobs["build-global-artifacts"] or errors << "missing build-global-artifacts job"
if build_global && !step_uploads?(build_global["steps"], "artifacts-dist-manifests")
  errors << "build-global-artifacts missing artifacts-dist-manifests upload"
end

finalize = jobs["custom-finalize-release-artifacts"] or errors << "missing custom-finalize-release-artifacts job"
if finalize
  needs = Array(finalize["needs"])
  unless needs.include?("custom-package-macos-universal")
    errors << "custom-finalize-release-artifacts must need custom-package-macos-universal"
  end
  inputs = finalize.dig("with") || {}
  unless inputs.key?("release_version")
    errors << "custom-finalize-release-artifacts missing release_version input"
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
end

errors << "announce job should be omitted (release is created in host)" if jobs.key?("announce")

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
