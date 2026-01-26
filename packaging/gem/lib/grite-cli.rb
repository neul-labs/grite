# frozen_string_literal: true

require 'net/http'
require 'uri'
require 'fileutils'
require 'tmpdir'
require 'rubygems/package'
require 'zlib'

module GriteCli
  VERSION = '0.3.0'
  REPO = 'neul-labs/grite'

  class << self
    def cache_dir
      base = if Gem.win_platform?
               ENV.fetch('LOCALAPPDATA', File.join(Dir.home, 'AppData', 'Local'))
             elsif RUBY_PLATFORM.include?('darwin')
               File.join(Dir.home, 'Library', 'Caches')
             else
               ENV.fetch('XDG_CACHE_HOME', File.join(Dir.home, '.cache'))
             end

      dir = File.join(base, 'grite-cli')
      FileUtils.mkdir_p(dir)
      dir
    end

    def platform
      os = case RbConfig::CONFIG['host_os']
           when /darwin/i then 'apple-darwin'
           when /linux/i then 'unknown-linux-gnu'
           when /mswin|mingw|cygwin/i then 'pc-windows-msvc'
           else raise "Unsupported OS: #{RbConfig::CONFIG['host_os']}"
           end

      arch = case RbConfig::CONFIG['host_cpu']
             when /x86_64|amd64/i then 'x86_64'
             when /aarch64|arm64/i then 'aarch64'
             else raise "Unsupported architecture: #{RbConfig::CONFIG['host_cpu']}"
             end

      # Use universal binary for macOS
      return 'universal-apple-darwin' if os == 'apple-darwin'

      "#{arch}-#{os}"
    end

    def archive_ext
      Gem.win_platform? ? '.zip' : '.tar.gz'
    end

    def binary_ext
      Gem.win_platform? ? '.exe' : ''
    end

    def binary_path(name = 'grite')
      version_dir = File.join(cache_dir, VERSION)
      binary = File.join(version_dir, "#{name}#{binary_ext}")

      unless File.exist?(binary)
        download_binary(version_dir)
      end

      raise "Binary not found: #{binary}" unless File.exist?(binary)

      binary
    end

    def download_binary(dest_dir)
      plat = platform
      ext = archive_ext
      archive_name = "grite-#{VERSION}-#{plat}#{ext}"
      url = "https://github.com/#{REPO}/releases/download/v#{VERSION}/#{archive_name}"

      puts "Downloading grite v#{VERSION} for #{plat}..."

      Dir.mktmpdir do |temp_dir|
        archive_path = File.join(temp_dir, archive_name)

        # Download with redirect following
        download_file(url, archive_path)

        # Extract
        if ext == '.tar.gz'
          extract_tar_gz(archive_path, temp_dir)
        else
          extract_zip(archive_path, temp_dir)
        end

        # Find extracted directory
        extracted_dir = Dir.glob(File.join(temp_dir, 'grite-*')).find { |d| File.directory?(d) }
        raise 'Could not find extracted directory' unless extracted_dir

        # Create destination
        FileUtils.mkdir_p(dest_dir)

        # Copy binaries
        %w[grite grite-daemon].each do |binary|
          src = File.join(extracted_dir, "#{binary}#{binary_ext}")
          dst = File.join(dest_dir, "#{binary}#{binary_ext}")
          FileUtils.cp(src, dst)
          FileUtils.chmod(0o755, dst) unless Gem.win_platform?
        end
      end

      puts "Successfully installed grite to #{dest_dir}"
    end

    private

    def download_file(url, dest)
      uri = URI.parse(url)
      response = nil

      loop do
        http = Net::HTTP.new(uri.host, uri.port)
        http.use_ssl = (uri.scheme == 'https')
        request = Net::HTTP::Get.new(uri.request_uri)
        response = http.request(request)

        case response
        when Net::HTTPRedirection
          uri = URI.parse(response['location'])
        when Net::HTTPSuccess
          break
        else
          raise "Download failed: #{response.code} #{response.message}"
        end
      end

      File.binwrite(dest, response.body)
    end

    def extract_tar_gz(archive_path, dest_dir)
      Gem::Package::TarReader.new(Zlib::GzipReader.open(archive_path)) do |tar|
        tar.each do |entry|
          dest = File.join(dest_dir, entry.full_name)
          if entry.directory?
            FileUtils.mkdir_p(dest)
          else
            FileUtils.mkdir_p(File.dirname(dest))
            File.binwrite(dest, entry.read)
          end
        end
      end
    end

    def extract_zip(archive_path, dest_dir)
      require 'zip'
      Zip::File.open(archive_path) do |zip_file|
        zip_file.each do |entry|
          dest = File.join(dest_dir, entry.name)
          FileUtils.mkdir_p(File.dirname(dest))
          entry.extract(dest) { true }
        end
      end
    rescue LoadError
      # Fallback to system unzip
      system("unzip -q '#{archive_path}' -d '#{dest_dir}'") || raise('unzip failed')
    end
  end
end
