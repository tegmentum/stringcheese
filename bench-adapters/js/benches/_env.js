// Side-effect module: env-var tweaks that must be set BEFORE any of the
// noisy ecosystem packages load.
//
// The `natural` package transitively pulls in `dotenvx`, which prints
// a chatty banner (`◇ injected env (0) from .env // tip: ⌘ suppress
// logs { quiet: true }`) on every module load unless
// `DOTENV_CONFIG_QUIET=true` is set at the moment `dotenvx` initialises.
//
// ES-module hoisting means `process.env.DOTENV_CONFIG_QUIET = "true"`
// written above an `import natural from "natural"` line does NOT run
// first — imports are evaluated in module-graph order, before any
// statement in the current module. Setting the env var here, then
// importing this file (`import "./_env.js"`) *before* the `natural`
// import in the same source-order-preserving graph, is what actually
// gets the assignment done in time.
//
// See ECMAScript §16.2.1.4 InnerModuleEvaluation: sibling requested
// modules are evaluated in source order.

if (process.env.DOTENV_CONFIG_QUIET === undefined) {
  process.env.DOTENV_CONFIG_QUIET = "true";
}
