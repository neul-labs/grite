Gem::Specification.new do |spec|
  spec.name          = 'grite-cli'
  spec.version       = '0.3.0'
  spec.authors       = ['Neul Labs']
  spec.email         = ['hello@neul.com']

  spec.summary       = 'Git-backed issue tracking for coding agents and humans'
  spec.description   = 'Grite is a repo-local, git-backed issue/task system designed for coding agents and humans. It maintains an append-only event log in git refs, builds a fast local materialized view, and never writes tracked state into the working tree.'
  spec.homepage      = 'https://github.com/neul-labs/grite'
  spec.license       = 'MIT'

  spec.required_ruby_version = '>= 2.7.0'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/neul-labs/grite'
  spec.metadata['changelog_uri'] = 'https://github.com/neul-labs/grite/releases'

  spec.files = Dir.chdir(__dir__) do
    `git ls-files -z`.split("\x0").reject do |f|
      (File.expand_path(f) == __FILE__) || f.start_with?('bin/', 'test/', 'spec/', 'features/', '.git', 'appveyor', 'Gemfile')
    end
  end

  spec.bindir = 'exe'
  spec.executables = ['grite', 'grite-daemon']
  spec.require_paths = ['lib']
end
