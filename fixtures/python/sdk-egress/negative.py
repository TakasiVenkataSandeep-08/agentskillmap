"""Local objects with SDK-shaped method names. None reaches a network."""


def persist(db, form):
    # Must NOT fire: `.create(` alone is an ORM method, which is why the LLM
    # chain is matched three levels deep rather than on the verb.
    user = db.users.create(name=form["name"])
    row = db.records.create(id=1)
    return [user, row]


def draft(thread):
    # Must NOT fire: a local message store.
    return thread.messages.create(body="hello")


def size(chat):
    # Must NOT fire: `completions` as data rather than as a call.
    return len(chat.completions)
