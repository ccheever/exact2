# Trips

An iPhone trip journal that uses the platform's location, photo library, map and local notification packages.

Data: `data/trips.json`, 80 seeded entries. Each entry has an id, title, date, location, photo filename, note and
reminder flag. Ship the data as the starting journal.

1. The main screen lists entries newest first, showing title, date, location and a thumbnail. Search filters by title,
   location and note as the person types.
2. Tapping an entry opens its detail screen with the note, photo and a map centered on the entry's location. The map
   uses the map package; do not draw a map image as a substitute.
3. Add and edit an entry in a sheet. The form takes a title, note, date and location chosen from the current location
   package. The photo field chooses an image from the photo library package and shows its thumbnail.
4. A saved entry can schedule a local notification for its date and can cancel that reminder. Request each permission
   when the person first uses its feature, and show a useful denied-permission state.
5. Delete an entry from the list or detail screen. Changes survive relaunching the app.
