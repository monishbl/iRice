const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const source = fs.readFileSync(path.join(__dirname, '../shell/services/AiModels.js'), 'utf8').replace(/^\.pragma library\s*/, '');
const api = vm.runInNewContext(source + '; ({normalize, validate, read, protocol, credentialProvider, sameConnection, migrate, buildRequest})');
const plain = value => JSON.parse(JSON.stringify(value));
const settings = { systemPrompt: 'Test system', showThinking: true, effort: 'medium', ollamaUrl: 'http://localhost:11434' };
const turns = [{role: 'user', text: 'Test prompt'}, {role: 'assistant', text: 'Test reply'}];
const model = (provider, extra = {}) => api.normalize({id: 'user-chosen-model', provider, ...extra});
const request = (m, opts = {}, history = turns) => api.buildRequest(m, history, opts, settings, 'fixture-secret');

test('no catalogue is inserted for fresh configurations', () => {
    assert.deepEqual(plain(api.migrate('', [], settings.ollamaUrl)), []);
    assert.deepEqual(plain(api.read([])), []);
});
test('legacy active choice and extra models are imported without all old presets', () => {
    const out = api.migrate('claude-sonnet-5-5', [{id:'existing', name:'My server', provider:'ollama'}], 'http://localhost:12000/');
    assert.equal(out.length, 2);
    assert.equal(out[0].id, 'claude-sonnet-5-5');
    assert.equal(out[0].thinking, true);
    assert.equal(out[0].maxTokens, 64000);
    assert.equal(out[1].endpoint, 'http://localhost:12000/v1');
    assert.equal(api.migrate('existing', [{id:'existing', provider:'gemini'}], '').length, 1);
});
test('same model IDs have separate identities across providers and connections', () => {
    const a = model('aistudio');
    const b = model('vertex');
    const c = model('custom', {endpoint:'https://one.example/v1'});
    const d = model('custom', {endpoint:'https://two.example/v1'});
    assert.equal(new Set([a.key,b.key,c.key,d.key]).size, 4);
    assert.equal(api.read([a,b,c,d, c]).length, 4);
    assert.equal(model('aistudio', {name:'New label'}).key, a.key);
});
test('serialization roundtrip keeps models but never copies secret fields', () => {
    const m = model('vertex', {auth:'token', project:'my-project', location:'global', apiKey:'must-not-persist', token:'must-not-persist'});
    const raw = JSON.stringify(m);
    assert.ok(!raw.includes('must-not-persist'));
    assert.deepEqual(plain(api.read([raw])[0]), plain(m));
});
test('AI Studio uses its native streaming endpoint, key header and Gemini roles', () => {
    const m = model('aistudio', {id:'models/user-chosen-model'});
    const req = request(m, {maxTokens:2048});
    assert.equal(req.url, 'https://generativelanguage.googleapis.com/v1beta/models/user-chosen-model:streamGenerateContent?alt=sse');
    assert.equal(req.headers['x-goog-api-key'], 'fixture-secret');
    assert.equal(req.body.contents[1].role, 'model');
    assert.equal(req.body.generationConfig.maxOutputTokens, 2048);
    assert.equal(api.protocol(m), 'gemini');
});
test('Vertex Express uses an API key without requiring a project', () => {
    const m = model('vertex', {project:'ignored-project', location:'ignored-location'});
    assert.equal(api.validate(m), '');
    assert.equal(m.project, '');
    const req = request(m);
    assert.equal(req.url, 'https://aiplatform.googleapis.com/v1/publishers/google/models/user-chosen-model:streamGenerateContent?alt=sse');
    assert.equal(req.headers['x-goog-api-key'], 'fixture-secret');
    assert.equal(req.headers.authorization, undefined);
});
test('Vertex OAuth routes regional and global requests with separate token credentials', () => {
    for (const location of ['global','us-central1']) {
        const m = model('vertex', {auth:'token', project:'example-project', location});
        const req = request(m);
        const host = location === 'global' ? '' : location + '-';
        assert.equal(req.url, `https://${host}aiplatform.googleapis.com/v1/projects/example-project/locations/${location}/publishers/google/models/user-chosen-model:streamGenerateContent?alt=sse`);
        assert.equal(req.headers.authorization, 'Bearer fixture-secret');
        assert.equal(req.headers['x-goog-api-key'], undefined);
        assert.equal(api.credentialProvider(m), 'vertex-oauth');
        assert.equal(api.protocol(m), 'gemini');
    }
    assert.match(api.validate(model('vertex',{auth:'token'})), /project/i);
});
test('OpenRouter keeps provider/model ID and uses bearer authentication', () => {
    const req = request(model('openrouter',{id:'publisher/model-selected-by-user'}));
    assert.equal(req.url, 'https://openrouter.ai/api/v1/chat/completions');
    assert.equal(req.body.model, 'publisher/model-selected-by-user');
    assert.equal(req.headers.authorization, 'Bearer fixture-secret');
});
test('OpenAI-compatible, local Ollama and unauthenticated servers use selected base URLs', () => {
    assert.equal(request(model('openai')).url, 'https://api.openai.com/v1/chat/completions');
    const local = request(model('ollama'));
    assert.equal(local.url, 'http://localhost:11434/v1/chat/completions');
    assert.equal(local.headers.authorization, undefined);
    const custom = request(model('custom',{endpoint:'http://localhost:8000/v1/',auth:'none'}));
    assert.equal(custom.url, 'http://localhost:8000/v1/chat/completions');
    assert.equal(custom.headers.authorization, undefined);
});
test('arbitrary Anthropic models are not forced to use adaptive thinking or beta flags', () => {
    const req = request(model('anthropic'));
    assert.equal(req.body.max_tokens, 4096);
    assert.equal(req.body.thinking, undefined);
    assert.equal(req.body.output_config, undefined);
    assert.equal(req.headers['anthropic-beta'], undefined);
    assert.equal(req.headers['x-api-key'], 'fixture-secret');
    const thinking = request(model('anthropic',{thinking:true,maxTokens:8192}));
    assert.equal(thinking.body.thinking.type, 'adaptive');
    const quick = request(model('anthropic',{thinking:true}),{thinking:false,maxTokens:2048});
    assert.equal(quick.body.thinking, undefined);
    assert.equal(quick.body.max_tokens, 2048);
});
test('screenshots use the selected provider format', () => {
    const history = [{role:'user',text:'Describe',image:{mediaType:'image/png',data:'fixture-base64'}}];
    assert.equal(request(model('aistudio'),{},history).body.contents[0].parts[0].inline_data.data, 'fixture-base64');
    assert.equal(request(model('vertex'),{},history).body.contents[0].parts[0].inline_data.data, 'fixture-base64');
    assert.equal(request(model('anthropic'),{},history).body.messages[0].content[0].source.data, 'fixture-base64');
    assert.equal(request(model('openrouter'),{},history).body.messages[1].content[0].image_url.url, 'data:image/png;base64,fixture-base64');
});
test('keys only travel over plain HTTP to this computer', () => {
    for (const ep of ['http://localhost:8000/v1', 'http://127.0.0.1:8000/v1', 'http://[::1]:8000/v1'])
        assert.equal(api.validate(model('custom',{endpoint:ep})), '', ep);
    assert.match(api.validate(model('custom',{endpoint:'http://192.168.1.5:8000/v1'})), /https:\/\//);
    assert.match(api.validate(model('custom',{endpoint:'http://localhost.evil.example/v1'})), /https:\/\//);
    assert.equal(api.validate(model('custom',{endpoint:'http://192.168.1.5:8000/v1',auth:'none'})), '');
});
test('invalid configuration is rejected before building a request', () => {
    for (const raw of [
        {provider:'aistudio', id:''},
        {provider:'not-supported', id:'test'},
        {provider:'vertex', id:'bad/path'},
        {provider:'custom', id:'test', endpoint:''},
        {provider:'custom', id:'test', endpoint:'https://user:pass@host/v1'},
        {provider:'custom', id:'test', endpoint:'https://host/v1?key=secret'},
        {provider:'custom', id:'test', endpoint:'file:///tmp/model'},
        {provider:'custom', id:'test', endpoint:'http://models.example/v1'},
        {provider:'anthropic',id:'test',maxTokens:'NaN'},
        {provider:'vertex',id:'test',auth:'token',project:'p',location:'../bad'},
    ]) {
        const m=api.normalize(raw);
        assert.notEqual(api.validate(m), '', JSON.stringify(raw));
        assert.throws(()=>request(m));
    }
    assert.equal(api.read(['bad json',null,{provider:'unknown',id:'test'}]).length,0);
});

test('credential retention is limited to edits on the same connection', () => {
    const original = model('custom',{endpoint:'https://one.example/v1'});
    assert.equal(api.sameConnection(original, model('custom',{id:'renamed-model',endpoint:'https://one.example/v1'})), true);
    assert.equal(api.sameConnection(original, model('custom',{endpoint:'https://two.example/v1'})), false);
    assert.equal(api.sameConnection(model('vertex',{auth:'token',project:'one'}), model('vertex',{auth:'token',project:'two'})), false);
    assert.equal(api.sameConnection(model('vertex'), model('vertex',{auth:'token',project:'one'})), false);
});
