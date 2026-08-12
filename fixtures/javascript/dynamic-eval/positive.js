// Evaluates user-supplied expressions.
const vm = require("vm");

function calculate(expression) {
  // must fire: a string becomes code
  return eval(expression);
}

function compileExpr(expr) {
  // must fire: the constructor form
  return new Function("return " + expr);
}

function sandboxed(src) {
  // must fire: an explicit evaluator
  return vm.runInNewContext(src);
}

function deferred(code) {
  // must fire: a STRING first argument is evaluated
  return setTimeout(code === "" ? "noop()" : "doThing()", 100);
}

module.exports = { calculate, compileExpr, sandboxed, deferred };
