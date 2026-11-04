/*
 * The films' footage (capture/make-clips.sh writes clips/ and plates/ from the
 * takes capture/capture.sh records in the hidden sandbox). Regions are where
 * each clip was recorded, in screen points (1920x1080), so it lands on its
 * plate exactly.
 */
const C = (n) => `clips/${n}.mp4`;
export const CLIPS = {
	'g-island': C('g-island'), 'm-island': C('m-island'),
	'g-cc': C('g-cc'), 'm-cc': C('m-cc'),
	'g-ask': C('g-ask'), 'm-ask': C('m-ask'),
	'g-chats': C('g-chats'), 'm-chats': C('m-chats'),
	'g-livetext': C('g-livetext'), 'm-livetext': C('m-livetext'),
	'g-share': C('g-share'), 'm-share': C('m-share'),
	'full-g-intro': C('full-g-intro'), 'full-switch': C('full-switch'),
	'full-g-circle': C('full-g-circle'), 'full-m-circle': C('full-m-circle'),
	'full-g-colour': C('full-g-colour'), 'full-m-colour': C('full-m-colour'),
	/* film time → clip time maps, where a take needs trimming or speeding (per film) */
	map: {}
};
export const REGION = {
	'g-island': [560, 0, 800, 200], 'm-island': [560, 0, 800, 200],
	'g-cc': [1480, 20, 440, 540], 'm-cc': [1480, 20, 440, 540],
	'g-ask': [0, 0, 470, 1080], 'm-ask': [0, 0, 470, 1080],
	'g-chats': [0, 0, 470, 1080], 'm-chats': [0, 0, 470, 1080],
	'g-livetext': [260, 90, 1400, 950], 'm-livetext': [260, 90, 1400, 950],
	'g-share': [560, 280, 800, 520], 'm-share': [560, 280, 800, 520]
};
/* the full-screen takes cover their plate completely */
for (const n of ['full-g-intro', 'full-switch', 'full-g-circle', 'full-m-circle', 'full-g-colour', 'full-m-colour']) REGION[n] = [0, 0, 1920, 1080];
export const PLATES = {
	g: 'plates/g-desktop.jpg',
	m: 'plates/m-desktop.jpg',
	gfull: 'plates/full-g-desktop.jpg',
	mfull: 'plates/full-m-desktop.jpg'
};
