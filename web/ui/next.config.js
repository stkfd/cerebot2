/* eslint-disable @typescript-eslint/no-var-requires */

const css = require('@zeit/next-css');

module.exports = css({
    env: {
        apiBaseUrl: "http://localhost:3001/api/1.0"
    },
});
