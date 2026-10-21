// The two clocks of SPEC "Clocks, live ticks, freeze", shared through context.
import { createContext, useContext } from 'react';
import type { SharedValue } from 'react-native-reanimated';

/** Live clock s: whole seconds since launch, ticking at 1 Hz only in live mode (else 0). */
export const LiveClock = createContext(0);
export const useLive = () => useContext(LiveClock);

/** Motion clock t: seconds since launch, continuous (a Reanimated shared value, UI thread). */
export const MotionClock = createContext<SharedValue<number> | null>(null);
export const useMotion = () => useContext(MotionClock)!;

/** Content width C = min(list width − 32, 600). */
export const Width = createContext(600);
export const useC = () => useContext(Width);

