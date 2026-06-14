// A tiny stub router, added by the `with-router` feature of the
// hello-world template. This file exists so the import injected into
// `index.js` (`const router = require('./router');`) resolves — a feature
// that injects a `require` must also ship the module it references.
module.exports = {
  routes: {},
  register(path, handler) {
    this.routes[path] = handler;
    return this;
  },
};
