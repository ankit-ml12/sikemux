const { withAppBuildGradle } = require('expo/config-plugins');

// Release builds are signed with the Play upload key, which scripts/run.mjs hands over through
// these two variables, so neither the keystore nor its password is ever written into the project.
// Without them a release build is left unsigned: the Mobile release workflow signs it in a job that
// never runs the build.
const MARKER = '// Signed by plugins/release-signing.js';

module.exports = (config) =>
  withAppBuildGradle(config, (config) => {
    if (!config.modResults.contents.includes(MARKER)) {
      config.modResults.contents += `
${MARKER}
android {
    signingConfigs {
        upload {
            storeFile file(System.getenv('SIKEMUX_UPLOAD_KEYSTORE') ?: 'upload.keystore')
            storePassword System.getenv('SIKEMUX_UPLOAD_PASSWORD')
            keyAlias 'upload'
            keyPassword System.getenv('SIKEMUX_UPLOAD_PASSWORD')
        }
    }
    buildTypes {
        release {
            signingConfig System.getenv('SIKEMUX_UPLOAD_KEYSTORE') ? signingConfigs.upload : null
        }
    }
}
`;
    }
    return config;
  });
