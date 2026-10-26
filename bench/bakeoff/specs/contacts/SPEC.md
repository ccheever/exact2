# Contacts

An iPhone contacts directory.

Data: `data/contacts.json`, 320 people (`id`, `first`, `last`, `company`, `phone`, `email`, `birthday`, `favorite`). Ship it with the app as the starting contacts.

1. The main screen lists every contact, sorted by last name, in sections by the last name's first letter, with a section index to jump between letters.
2. A search field filters by name, company, phone or email as you type; clearing it restores the full list.
3. Tapping a contact opens their details: name, company, phone, email, birthday when known. Phone and email look tappable.
4. A contact can be marked and unmarked as a favorite from the details screen. Favorites appear in their own section at the top of the list.
5. Add a contact (first name, last name, company, phone, email); a contact needs a first or last name. Edit any contact the same way.
6. Delete a contact from the list and from its details screen.
7. Changes survive relaunching the app.
