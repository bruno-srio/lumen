# How this project is written

This is a working note. Rewrite it in your own words before you use it as the Flathub disclosure. The text that goes in the Flathub submission is yours to write. An assistant should not draft that submission, open the pull request, or write its commits, description, or replies.

Cursor, and the models behind it, drafted most of what is in this repository. That includes the GTK app, the Yeelight protocol crate, the simulator, the tests, the docs, the helper scripts, and the Flatpak files under `build-aux/`. I decided what the app should do, rejected behavior I did not want, and tested it on the simulator and on real bulbs. I have not audited every generated line.

There is no honest percentage. "Most of the application, its tests, its docs, and the packaging files in this repository" is the description I would stand behind until I correct it.

Help that never landed in the tree, such as a debugging conversation, is not something Flathub asks me to disclose by itself. The drafts that were committed are.

The JSON files in `build-aux/` were drafted with Cursor. They are for local builds. They are not the Flathub manifest. Flathub's rules say the manifest it builds from must not be AI-generated or AI-assisted, and disclosure does not waive that. I will write that manifest myself. The old `~/lumen-flathub` checkout was prepared with AI help, so I will not submit it as it is.

Flathub can still reject the app after a complete disclosure. That is their call.

The current policy is the generative AI section of [Flathub's requirements](https://docs.flathub.org/docs/for-app-authors/requirements). I checked it on 27 Nov 2026. Read it again before submitting, because it can change.
