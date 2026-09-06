import ding from '../assets/sounds/ding.mp3?url';
import quotaAlert from '../assets/sounds/quota-alert.mp3?url';
import quotaBattery from '../assets/sounds/quota-low-battery.mp3?url';

export type Sound = 'completion' | 'quotaAlert' | 'quotaBattery';

const players: Record<Sound, HTMLAudioElement> = {
  completion: new Audio(ding),
  quotaAlert: new Audio(quotaAlert),
  quotaBattery: new Audio(quotaBattery),
};

for (const player of Object.values(players)) player.preload = 'auto';

export async function playSound(sound: Sound) {
  const player = players[sound];
  player.pause();
  player.currentTime = 0;
  await player.play();
}
