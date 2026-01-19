const path = require('path');
const os = require('os');

const binExt = os.platform() === 'win32' ? '.exe' : '';

module.exports = {
  gritPath: path.join(__dirname, 'bin', `grit${binExt}`),
  gritdPath: path.join(__dirname, 'bin', `gritd${binExt}`),
};
