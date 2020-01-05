const css = require('@zeit/next-css');

module.exports = css({
    env: {
        apiBaseUrl: "http://localhost:3001/api/1.0"
    },
    webpack: (config, { buildId, dev, isServer, defaultLoaders, webpack }) => {
        return config
    },
    webpackDevMiddleware: config => {
        return config
    },
});
