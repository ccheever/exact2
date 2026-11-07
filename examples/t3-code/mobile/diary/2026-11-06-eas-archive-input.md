# EAS archive input

## Rough

The first TestFlight build attempt compressed a 409 MB checkout. The archive included local `target` output and installed dependency trees even though the EAS production hook installs dependencies and builds the app on the worker.

## Lean in

The repository now has a root `.easignore` that excludes generated targets, dependency directories, Exact scratch state, build products and logs. This keeps the delivery input limited to source and checked-in build configuration.

## Needed

The next EAS retry needs network access to upload the smaller archive. This configuration change does not alter the app bundle.

## Checkpoints

- The initial EAS attempt reached archive compression and failed while uploading the 409 MB tarball.
- The retry was blocked by a transient GraphQL connection failure before an EAS build ID was issued.
