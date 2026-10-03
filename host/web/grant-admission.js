// Lazy host modules and the JavaScript target use the same admission code as
// the pre-pixel wasm host, whose two-module boot contract owns the definitions.
export {
  admitsNetwork,
  admitsSecret,
  coversPath,
  grantError,
  hasGrant,
  rawGrantText,
  scopedGrantSet,
  unionGrantSets,
} from './navigation.js';
