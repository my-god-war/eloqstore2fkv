# Review records

`issue/` is the review board during migration:

- `new/`: Entry for new comments.
- `todo/`: Adopted, pending development.
- `done/`: Developed and has test evidence, pending review.
- `reviewed/`: Archive reviewed.
- `reject/`: If not adopted, please state the reasons.

See `issue/new.md`, `issue/run.md`, `issue/review.md` for the process.

The review focuses on behavioral differences, format compatibility, performance, and code structure between fkv and C++ implementations. One opinion per
files to avoid bulk moves.