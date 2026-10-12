// The renderer state (F05): the pure reducer, slice registration, and the zustand store.
export * from './reducer';
export { collectSlices, registeredSlices } from './slice-registry';
export { useCoreStore, type CoreStore } from './store';
