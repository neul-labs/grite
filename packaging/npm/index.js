const path = require('path');
const os = require('os');

const binExt = os.platform() === 'win32' ? '.exe' : '';

module.exports = {
  gritePath: path.join(__dirname, 'bin', `grite${binExt}`),
  griteDaemonPath: path.join(__dirname, 'bin', `grite-daemon${binExt}`),
};
