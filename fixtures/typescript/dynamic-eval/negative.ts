// Scores a model, queries a database, and schedules work. None evaluates a
// string.

function score(model, batch) {
  // Must NOT fire: a mode switch, not an evaluator.
  model.eval();
  return model(batch);
}

function query(cursor, sql) {
  // Must NOT fire: a cursor method; also how every regex in this language runs.
  return cursor.exec(sql);
}

function later(fn) {
  // Must NOT fire: the function form evaluates nothing, and it is the form
  // nearly every asynchronous skill uses. Matching the callee name alone would
  // report all of them.
  return setTimeout(fn, 100);
}

function parse(text) {
  // Must NOT fire: deserialization is not evaluation.
  return JSON.parse(text);
}

module.exports = { score, query, later, parse };
