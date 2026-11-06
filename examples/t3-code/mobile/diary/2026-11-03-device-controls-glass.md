# Device preview controls use the source glass surface

The device preview route had the source control geometry but rendered its header as a flat secondary-color capsule and removed the overlay immediately when controls were hidden. The pinned source uses `GlassSurface` for the header and fades the full control layer while leaving a separate handle available to reveal it.

The Contract now mounts the existing `t3-composer-material` leaf behind the device controls, uses the source 32-point glass radius and shadow, and transitions the control layer's opacity over 160 ms. Hidden controls disable pointer events and accessibility while the reveal handle remains mounted. The route passes the current appearance so the native material selects its light or dark effect.

Focused coverage checks the material, animation, pointer ownership and reveal-handle declarations. Contract compilation remains the authoritative validation for the new native leaf and props.
