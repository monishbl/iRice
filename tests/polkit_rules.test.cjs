// Tessera's polkit rule (shell/assets/polkit/49-tessera.rules), run in Node
// against a fake polkit: which of the root helper's actions pass without a
// password, for whom. NOT_HANDLED means the action's default (the password).
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const HELPER = '/usr/lib/tessera/tessera-root';
const Result = { NO: 'no', YES: 'yes', AUTH_SELF: 'auth_self', AUTH_SELF_KEEP: 'auth_self_keep', AUTH_ADMIN: 'auth_admin', AUTH_ADMIN_KEEP: 'auth_admin_keep', NOT_HANDLED: null };
const rules = [];
vm.runInContext(fs.readFileSync(__dirname + '/../shell/assets/polkit/49-tessera.rules', 'utf8'),
    vm.createContext({ polkit: { Result, addRule: f => rules.push(f), log: () => {} } }));

// pkexec's details for `pkexec <program> <args>` (its command_line: argv joined by spaces)
const action = (id, program, args) => ({
    id,
    lookup: key => ({ program, command_line: [program, ...args].join(' ') })[key],
});
const subject = (over = {}) => Object.assign({ active: true, local: true, groups: ['wheel', 'users'] }, over, {
    isInGroup(g) { return this.groups.includes(g); },
});
const decide = (a, s = subject()) => {
    for (const r of rules) {
        const v = r(a, s);
        if (v !== null && v !== undefined)
            return v;
    }
    return null;
};

test('two rules: the root helper, and the fingerprint reader restart', () => {
    assert.equal(rules.length, 2);
});

// systemd's details for a unit job (RestartUnit and friends)
const unitJob = (unit, verb) => ({
    id: 'org.freedesktop.systemd1.manage-units',
    lookup: key => ({ unit, verb })[key],
});
const person = (over = {}) => subject(Object.assign({ user: 'monish', groups: ['users'] }, over));

test('the person at the computer restarts fprintd without a password', () => {
    assert.equal(decide(unitJob('fprintd.service', 'restart'), person()), 'yes');
    // (not only administrators: it's their own reader)
    assert.equal(decide(unitJob('fprintd.service', 'restart'), person({ groups: ['wheel'] })), 'yes');
});

test('only fprintd, only a restart, only someone at the computer', () => {
    for (const verb of ['stop', 'start', 'reload', 'kill', 'enable', undefined])
        assert.equal(decide(unitJob('fprintd.service', verb), person()), null, String(verb));
    for (const unit of ['sshd.service', 'fprintd.socket', 'fprintd', 'polkit.service', undefined])
        assert.equal(decide(unitJob(unit, 'restart'), person()), null, String(unit));
    assert.equal(decide(unitJob('fprintd.service', 'restart'), person({ active: false })), null);
    assert.equal(decide(unitJob('fprintd.service', 'restart'), person({ local: false })), null);
    assert.equal(decide(unitJob('fprintd.service', 'restart'), person({ user: '' })), null);
    // (other systemd actions aren't touched)
    assert.equal(decide({ id: 'org.freedesktop.systemd1.manage-unit-files', lookup: () => undefined }, person()), null);
    assert.equal(decide({ id: 'org.freedesktop.systemd1.reload-daemon', lookup: () => undefined }, person()), null);
});

test('the small fixed jobs pass for an administrator at the computer', () => {
    assert.equal(decide(action('org.tessera.root.wallpaper', HELPER, ['wallpaper', '/home/a/Pictures/b.jpg'])), 'yes');
    assert.equal(decide(action('org.tessera.root.fix', HELPER, ['fix', 'avahi'])), 'yes');
    assert.equal(decide(action('org.tessera.root.login-look', HELPER, ['login-look', '{"style":"look"}'])), 'yes');
    assert.equal(decide(action('org.tessera.root.login-look', HELPER, ['wallpaper', '/x.jpg'])), null);
    assert.equal(decide(action('org.tessera.root.wifi-helper', HELPER, ['wifi-helper'])), null); // (asks: it makes Wi-Fi networks)
    // a path with spaces is fine: only the first word after the program counts
    assert.equal(decide(action('org.tessera.root.wallpaper', HELPER, ['wallpaper', '/home/a/My Pictures/b.jpg'])), 'yes');
});

test('updates and anything else keep the password', () => {
    assert.equal(decide(action('org.tessera.root.update', HELPER, ['update', '--system'])), null);
    assert.equal(decide(action('org.freedesktop.policykit.exec', HELPER, ['setup', '/x'])), null);
    assert.equal(decide(action('org.freedesktop.policykit.exec', '/usr/bin/bash', ['-i'])), null);
    assert.equal(decide(action('org.freedesktop.NetworkManager.settings.modify.system', undefined, [])), null);
});

test('not for someone away, remote, or not an administrator', () => {
    const a = action('org.tessera.root.fix', HELPER, ['fix', 'avahi']);
    assert.equal(decide(a, subject({ active: false })), null);
    assert.equal(decide(a, subject({ local: false })), null);
    assert.equal(decide(a, subject({ groups: ['users'] })), null);
});

test('the program and its first argument have to match the action', () => {
    assert.equal(decide(action('org.tessera.root.fix', '/home/a/tessera-root', ['fix', 'avahi'])), null);
    assert.equal(decide(action('org.tessera.root.fix', HELPER, ['update', 'fix'])), null);
    assert.equal(decide(action('org.tessera.root.wallpaper', HELPER, ['fix', 'avahi'])), null);
    assert.equal(decide({ id: 'org.tessera.root.fix', lookup: () => undefined }), null);
});
