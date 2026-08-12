// Local objects with method names that look like an SDK's. None reaches a
// network, and every one of them is a shape real code writes.

function persist(db, form) {
  // Must NOT fire: `.create(` on its own is an ORM method. This is why the LLM
  // chain is matched three levels deep rather than on the verb.
  const user = db.users.create({ name: form.name });
  const row = db.records.create({ id: 1 });
  return [user, row];
}

function draft(thread) {
  // Must NOT fire: a local message store. `messages.create` two levels deep was
  // considered and left out for exactly this shape.
  return thread.messages.create({ body: "hello" });
}

function render(template, chat) {
  // Must NOT fire: `chat` as a plain object, and `completions` as data.
  const history = chat.completions;
  return template.replace("{n}", String(history.length));
}

module.exports = { persist, draft, render };
