You are building an iPhone app, unattended: nobody will answer questions, so decide and keep going until it is done.

The app's specification is below. Its data files are in `./data/` in your working directory; ship them with the app
as the spec says. Make the app as good as an expert iOS developer's hand-built app: every feature in the spec working,
correct on the edge cases the spec names, and looking and behaving as an iPhone app should (navigation, lists, controls,
keyboard, dark mode, the largest text size). Test it on the simulator as you go.

- Use only the iOS simulator with udid `{SIM}` (it is yours alone). Do not boot, erase or use any other simulator.
- The app's bundle id is `{APP_ID}` and its name is `{APP_NAME}`.
- Work in your working directory. Do not read or write outside it except where the instructions for your toolkit
  below say to.
- Stop when the app is finished, built and installed on the simulator. Then write `NOTES.md` in your working
  directory: what was hard, what you had to look up or guess, anything you could not finish, and what would have
  made building this faster.

If this specification names a backend or realtime service, the harness has started its frozen local server at
`http://127.0.0.1:{SERVER_PORT}`. Use that origin in the app and leave the service running only through the build.

{STACK}

# Specification

{SPEC}
