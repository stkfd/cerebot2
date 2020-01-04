const css = require('@zeit/next-css');

module.exports = css({
    webpack: (config, { buildId, dev, isServer, defaultLoaders, webpack }) => {
        return config
    },
    webpackDevMiddleware: config => {
        return config
    },
});
