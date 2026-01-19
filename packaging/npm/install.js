#!/usr/bin/env node

const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const os = require('os');

const REPO = 'neul-labs/grit';
const pkg = require('./package.json');
const VERSION = pkg.version;

function getPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  const platformMap = {
    darwin: 'apple-darwin',
    linux: 'unknown-linux-gnu',
    win32: 'pc-windows-msvc',
  };

  const archMap = {
    x64: 'x86_64',
    arm64: 'aarch64',
  };

  const mappedPlatform = platformMap[platform];
  const mappedArch = archMap[arch];

  if (!mappedPlatform || !mappedArch) {
    throw new Error(`Unsupported platform: ${platform}-${arch}`);
  }

  // Use universal binary for macOS
  if (platform === 'darwin') {
    return 'universal-apple-darwin';
  }

  return `${mappedArch}-${mappedPlatform}`;
}

function getArchiveExt() {
  return os.platform() === 'win32' ? 'zip' : 'tar.gz';
}

function download(url) {
  return new Promise((resolve, reject) => {
    const request = (url) => {
      https.get(url, (response) => {
        if (response.statusCode === 302 || response.statusCode === 301) {
          request(response.headers.location);
          return;
        }

        if (response.statusCode !== 200) {
          reject(new Error(`Download failed: ${response.statusCode}`));
          return;
        }

        const chunks = [];
        response.on('data', (chunk) => chunks.push(chunk));
        response.on('end', () => resolve(Buffer.concat(chunks)));
        response.on('error', reject);
      }).on('error', reject);
    };
    request(url);
  });
}

async function extract(archivePath, destDir) {
  const ext = getArchiveExt();

  if (ext === 'tar.gz') {
    execSync(`tar -xzf "${archivePath}" -C "${destDir}"`);
  } else {
    // Windows - use PowerShell
    execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${destDir}' -Force"`);
  }
}

async function install() {
  try {
    const platform = getPlatform();
    const ext = getArchiveExt();
    const archiveName = `grit-${VERSION}-${platform}.${ext}`;
    const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${archiveName}`;

    console.log(`Downloading grit v${VERSION} for ${platform}...`);

    const data = await download(url);

    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'grit-'));
    const archivePath = path.join(tempDir, archiveName);

    fs.writeFileSync(archivePath, data);

    await extract(archivePath, tempDir);

    // Find extracted directory
    const extractedDir = fs.readdirSync(tempDir).find(f => f.startsWith('grit-') && fs.statSync(path.join(tempDir, f)).isDirectory());

    if (!extractedDir) {
      throw new Error('Could not find extracted directory');
    }

    const srcDir = path.join(tempDir, extractedDir);
    const binDir = path.join(__dirname, 'bin');

    // Create bin directory
    if (!fs.existsSync(binDir)) {
      fs.mkdirSync(binDir, { recursive: true });
    }

    // Copy binaries
    const isWindows = os.platform() === 'win32';
    const binExt = isWindows ? '.exe' : '';

    const gritSrc = path.join(srcDir, `grit${binExt}`);
    const gritdSrc = path.join(srcDir, `gritd${binExt}`);
    const gritDest = path.join(binDir, `grit${binExt}`);
    const gritdDest = path.join(binDir, `gritd${binExt}`);

    fs.copyFileSync(gritSrc, gritDest);
    fs.copyFileSync(gritdSrc, gritdDest);

    // Make executable on Unix
    if (!isWindows) {
      fs.chmodSync(gritDest, 0o755);
      fs.chmodSync(gritdDest, 0o755);
    }

    // Cleanup
    fs.rmSync(tempDir, { recursive: true, force: true });

    console.log('Successfully installed grit');
  } catch (error) {
    console.error('Installation failed:', error.message);
    console.error('You can install manually from: https://github.com/neul-labs/grit/releases');
    process.exit(1);
  }
}

install();
