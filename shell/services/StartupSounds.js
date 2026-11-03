.pragma library
// The sounds to log in to (Settings → Notifications → Logging in), each
// with its name and the style whose instrument plays it. Any can be chosen
// whichever style is on; "" is iRice's own chime, the default. The files
// are assets/sounds/startup/<id>.ogg (tools/make-sounds.py startups).

var styles = [
    { id: "soft", name: "Soft" },
    { id: "marimba", name: "Marimba" },
    { id: "keys", name: "Keys" },
    { id: "taps", name: "Taps" },
    { id: "minimal", name: "Minimal" },
    { id: "glass", name: "Glass" },
    { id: "material", name: "Material" }
]

// Each style: Rise, Hello, Dawn and Bell, under names of their own
var names = {
    "soft-rise": "Updraft", "soft-hello": "Good Morning", "soft-dawn": "Daybreak", "soft-bell": "Vesper",
    "marimba-rise": "Rainfall", "marimba-hello": "Greeting", "marimba-dawn": "Tropic", "marimba-bell": "Temple",
    "keys-rise": "Arpeggio", "keys-hello": "Lounge", "keys-dawn": "Late Night", "keys-bell": "Music Box",
    "taps-rise": "Footsteps", "taps-hello": "Knock", "taps-dawn": "Heartbeat", "taps-bell": "Drum Call",
    "minimal-rise": "Ladder", "minimal-hello": "Blink", "minimal-dawn": "Pulse", "minimal-bell": "Morse",
    "glass-rise": "Crystal", "glass-hello": "Prism", "glass-dawn": "Aurora", "glass-bell": "Wind Chime",
    "material-rise": "Bloom", "material-hello": "Wave", "material-dawn": "Halo", "material-bell": "Ripple"
}
var motifs = [
    { id: "rise", about: "A quick run up the chord that opens into it" },
    { id: "hello", about: "Two notes a fifth apart, blooming into a chord" },
    { id: "dawn", about: "A slow swell with notes sparkling over its peak" },
    { id: "bell", about: "Three chimes, down then a leap up" }
]

// Every choice, the original first: { id, name, style, about }
var all = [{ id: "", name: "iRice", style: "Original", about: "The chime iRice has always played" }].concat(
    styles.reduce((list, s) => list.concat(motifs.map(m => ({ id: s.id + "-" + m.id, name: names[s.id + "-" + m.id], style: s.name, about: m.about }))), []))

function find(id) {
    return all.find(c => c.id === id) ?? all[0]
}
