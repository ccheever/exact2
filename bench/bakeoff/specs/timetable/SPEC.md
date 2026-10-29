# Timetable

An iPhone transit departure board.

Data: `data/departures.json`: five lines (`id`, `name`, `color`), twenty stations, and about 6,000 departures (`station`, `line`, `destination`, `minute` after midnight, `platform`, `status`). Ship it.

1. The main screen lists every departure from the chosen station for the whole day in time order, grouped by hour with an hour heading that stays pinned at the top while its hour scrolls. Each row shows the time, the line as a coloured badge, the destination, the platform, and a delayed or cancelled status when not on time.
2. The chosen station is at the top; tapping it picks another from a searchable list of the twenty stations.
3. A Now button scrolls the list to the next departure after the current time; the list opens there.
4. A filter picks one line or all lines.
5. Departures already past today are dimmed.
6. Starring a departure adds it to a Saved list. Saved departures and the chosen station survive relaunching the app.
