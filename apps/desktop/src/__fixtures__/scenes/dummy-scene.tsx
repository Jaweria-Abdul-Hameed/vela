import type { SceneRegistration } from '../../registry';

export const scene: SceneRegistration = {
  id: 'dummy-scene',
  title: 'Dummy scene',
  order: 5,
  component: () => null,
};
