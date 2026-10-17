import { loadEngine } from "./load-engine.mjs";
import { createNodeHost } from "./node-host.mjs";

var loaded = loadEngine();
var host = createNodeHost({ pluginRoot: loaded.root, linkCli: true });
loaded.engine.setHost(host);
try {
  var code = await loaded.engine.main(process.argv.slice(2));
  process.exit(code || 0);
} catch (e) {
  console.error(e && e.stack ? e.stack : e);
  process.exit(1);
}
