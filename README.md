# Open EQMS (v03 skeleton)

This branch (main-new) contains the v03 snapshot of the Open EQMS UI shell. It includes a Flask entrypoint, static assets, and placeholder packages for future apps (FMEA, etc.). Core service modules are stubs to be filled in.

## What's here?
- app.py: Flask app with routes for / and /launch/<app_name>, renders 	emplates/index.html.
- static/: CSS/JS plus image and logo assets (renamed to short, git-friendly filenames).
- 	emplates/index.html: Simple dashboard UI referencing the static assets.
- apps/, eqms/: Package stubs with zero-length modules ready for implementation.

## Quick start
1) Create and activate a virtualenv.
2) Install deps: pip install -r requirements.txt
3) Run the dev server: lask --app app run --debug (or python app.py).

## Notes
- Asset filenames were shortened to avoid Windows path length issues.
- The core modules under eqms/core and apps/fmea are empty and need implementation before anything beyond the landing page will work.
- The default branch for this work is main-new; the existing remote main remains untouched.
