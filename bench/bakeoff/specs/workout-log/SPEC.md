# Workout Log

An iPhone workout log.

Data: `data/exercises.json`, a library of 120 exercises (`id`, `name`, `muscle`, `equipment`), and `data/history.json`, 60 past workouts (`date`, `name`, `exercises` with `sets` of `reps` and `weight` in pounds). Ship both.

1. A History screen lists past workouts newest first: name, date, number of exercises and total volume (reps × weight summed).
2. Tapping a workout shows its exercises and sets.
3. Start a new workout: name it, add exercises from the library (searchable, filterable by muscle), then add sets (reps and weight) to each exercise, edit or delete a set, and finish. Finishing adds it to History.
4. The library is browsable on its own screen, grouped by muscle.
5. An exercise's screen shows its best set (heaviest weight) and the workouts it appears in.
6. A workout in progress and the history survive relaunching the app.
