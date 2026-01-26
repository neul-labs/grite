const path = require('path');
const os = require('os');

const binExt = os.platform() === 'win32' ? '.exe' : '';

module.exports = {
  gritPath: path.join(__dirname, 'bin', `grit${binExt}`),
  grit-daemonPath: path.join(__dirname, 'bin', `grit-daemon${binExt}`),
};
