import approvalBell from '../assets/sounds/approval-bell.mp3?url';
import completionBell from '../assets/sounds/completion-bell.mp3?url';
import completionDing from '../assets/sounds/completion-ding.mp3?url';
import quotaAlert from '../assets/sounds/quota-alert.mp3?url';
import quotaBattery from '../assets/sounds/quota-low-battery.mp3?url';

export type Sound =
  'approvalBell' | 'completionBell' | 'completionDing' | 'quotaAlert' | 'quotaBattery';

const players: Record<Sound, HTMLAudioElement> = {
  approvalBell: new Audio(approvalBell),
  completionBell: new Audio(completionBell),
  completionDing: new Audio(completionDing),
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
