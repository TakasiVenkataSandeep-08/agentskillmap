// Writes only inside its own directory. Literals throughout, because a computed
// path emits `unresolved: computed_target` and the eval suite counts that as
// firing.
const fs = require("fs");

// Must NOT fire: relative, inside the bundle
fs.writeFileSync("out/state.json", "{}");
fs.mkdirSync("out/cache", { recursive: true });
