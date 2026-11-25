const METHODS = Object.freeze({
  getGrant: '0xbdd46005', committee: '0x00d472f9', observation: '0xd2342661',
  candidate: '0x6976fe95', refresh: '0x0e9f8e70', certify: '0xe7c78c54',
  configureGrant: '0x1d1551fd', configureCommittee: '0xd7dc75d5',
  startRound: '0x980d0fcf', claim: '0x4e71d92d',
  advanceEpoch: '0xdc0214a3', finalizeEpoch: '0xb4fd7d2e',
});
const DEFAULT_RPCS = [10087, 11087, 12087, 13087].map(port => `http://127.0.0.1:${port}/api`);
const DEFAULT_ADDRESS = '0x70e0ba845a1a0f2da3359c97e0285013525ffc49';
const STORAGE_KEY = 'grantstrace-frontend-v1';
const $ = id => document.getElementById(id);
const state = { grant: null, committee: null, observations: [null, null, null, null], candidate: null, account: null, local: null, busy: false };
let feedbackTimer;

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
function short(value, start = 10, end = 6) {
  const s = String(value ?? '');
  return s.length > start + end + 3 ? `${s.slice(0,start)}…${s.slice(-end)}` : s;
}
function config() {
  const address = $('contract-address').value.trim();
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) throw new Error('Enter a valid Lyquid contract address');
  const rpcs = DEFAULT_RPCS.map((_, i) => $('rpc-' + i).value.trim());
  for (const rpc of rpcs) {
    const url = new URL(rpc);
    if (!['http:', 'https:'].includes(url.protocol)) throw new Error('RPC URLs must use HTTP or HTTPS');
  }
  return {address, rpcs};
}
function saveConfig() {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(config())); } catch (_) { /* ignore incomplete input */ }
}
function loadConfig() {
  let saved;
  try { saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || 'null'); } catch (_) { saved = null; }
  $('contract-address').value = saved?.address || DEFAULT_ADDRESS;
  DEFAULT_RPCS.forEach((rpc, i) => $('rpc-' + i).value = saved?.rpcs?.[i] || rpc);
  $('grant-config').value = JSON.stringify({
    grant_id: 'grant-001', repo_owner: 'jqlang', repo_name: 'jq', repo_id: 5101141,
    window_start: 1748736000, window_end: 1754006400, min_stable_releases: 2,
    beneficiary: '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266', reward_credits: 100, rule_version: 1,
  }, null, 2);
}
function log(message, fail = false) {
  const li = document.createElement('li');
  li.textContent = `${new Date().toLocaleTimeString('en-US', {hour12:false})}  ${message}`;
  if (fail) li.className = 'fail';
  $('activity-log').prepend(li);
}
function feedback(message, kind) {
  clearTimeout(feedbackTimer);
  const element = $('action-feedback');
  element.textContent = message;
  element.className = `action-feedback ${kind}`;
  element.hidden = false;
  if (kind !== 'working') feedbackTimer = setTimeout(() => { element.hidden = true; }, kind === 'error' ? 12000 : 6000);
}
function setBusy(busy) {
  state.busy = busy;
  document.querySelectorAll('button').forEach(button => {
    if (button.id !== 'clear-log-button') button.disabled = busy;
  });
}
async function run(label, task) {
  if (state.busy) { feedback('Another action is in progress. Please wait.', 'working'); return; }
  setBusy(true);
  log(`${label} started`);
  feedback(`${label} in progress…`, 'working');
  try { await task(); log(`${label} completed`); feedback(`${label} completed`, 'success'); }
  catch (error) { log(`${label} failed: ${error.message}`, true); feedback(`${label} failed: ${error.message}`, 'error'); }
  finally { setBusy(false); }
}
function strip0x(value) { return value.startsWith('0x') ? value.slice(2) : value; }
function word(value) { return BigInt(value).toString(16).padStart(64, '0'); }
function encodeString(value) {
  const bytes = new TextEncoder().encode(value);
  const data = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
  return word(32) + word(bytes.length) + data.padEnd(Math.ceil(data.length / 64) * 64, '0');
}
function encodeAddress(value) {
  if (!/^0x[0-9a-fA-F]{40}$/.test(value)) throw new Error('Invalid LVM target address');
  return strip0x(value).toLowerCase().padStart(64, '0');
}
function encodeCall(selector, args = []) {
  if (args.length === 0) return selector;
  if (args.length === 1 && args[0].type === 'string') return selector + encodeString(args[0].value);
  // (string,address,bool): string is dynamic and follows three head words.
  if (args.length === 3 && args[0].type === 'string') {
    return selector + word(96) + encodeAddress(args[1].value) + word(args[2].value ? 1 : 0)
      + word(new TextEncoder().encode(args[0].value).length)
      + Array.from(new TextEncoder().encode(args[0].value), b => b.toString(16).padStart(2, '0')).join('').padEnd(Math.ceil(new TextEncoder().encode(args[0].value).length / 32) * 64, '0');
  }
  throw new Error('Unsupported ABI arguments');
}
function decodeString(hex) {
  const data = strip0x(hex || '');
  if (data.length < 128) throw new Error('RPC response is not an ABI string');
  const offset = Number(BigInt('0x' + data.slice(0, 64))) * 2;
  if (offset + 64 > data.length) throw new Error('Invalid RPC string offset');
  const length = Number(BigInt('0x' + data.slice(offset, offset + 64)));
  if (!Number.isSafeInteger(length) || length > 1_000_000 || offset + 64 + length * 2 > data.length) throw new Error('Invalid RPC string length');
  const payload = data.slice(offset + 64, offset + 64 + length * 2);
  return new TextDecoder().decode(Uint8Array.from(payload.match(/../g) || [], byte => parseInt(byte, 16)));
}
// JSON.parse rounds u64 values. Quote only unsafe integer tokens before parsing so IDs stay exact.
function parseJsonLossless(input) {
  let output = '', inString = false, escaped = false;
  for (let i = 0; i < input.length; i++) {
    const c = input[i];
    if (inString) { output += c; if (escaped) escaped = false; else if (c === '\\') escaped = true; else if (c === '"') inString = false; continue; }
    if (c === '"') { inString = true; output += c; continue; }
    if (c === '-' || (c >= '0' && c <= '9')) {
      const match = input.slice(i).match(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/);
      if (match) {
        const token = match[0];
        output += /^-?\d+$/.test(token) && !Number.isSafeInteger(Number(token)) ? `"${token}"` : token;
        i += token.length - 1; continue;
      }
    }
    output += c;
  }
  return JSON.parse(output);
}
async function rpc(url, method, params) {
  const response = await fetch(url, {method:'POST', headers:{'content-type':'application/json'}, body:JSON.stringify({jsonrpc:'2.0', id:1, method, params})});
  if (!response.ok) throw new Error(`RPC HTTP ${response.status}`);
  const result = await response.json();
  if (result.error) throw new Error(result.error.message || `RPC error ${result.error.code}`);
  return result.result;
}
async function read(index, selector) {
  const {address, rpcs} = config();
  const result = await rpc(rpcs[index], 'eth_call', [{to:address, data:selector}, 'latest']);
  const envelope = parseJsonLossless(decodeString(result));
  if (!envelope || typeof envelope.ok !== 'boolean') throw new Error('Invalid API response format');
  return envelope;
}
function dataOf(envelope) {
  if (!envelope.ok) throw new Error(`${envelope.error?.code || 'UNKNOWN'}: ${envelope.error?.message || 'Operation failed'}`);
  return envelope.data;
}
function pill(text, kind = 'muted') { return `<span class="badge ${kind}">${esc(text)}</span>`; }
function row(label, value) { return `<div class="data-row"><span>${esc(label)}</span><strong>${esc(value)}</strong></div>`; }
function renderMetrics() {
  const grant = state.grant?.state;
  const snapshot = grant?.snapshot;
  $('metric-round').textContent = grant?.round_id ?? '—';
  $('metric-stable').textContent = snapshot?.candidate?.stable_count ?? '—';
  $('metric-signers').textContent = snapshot ? `${snapshot.signer_node_ids.length} / 4` : '—';
  $('metric-eligible').textContent = !state.grant ? '—' : !snapshot ? 'Awaiting Certification' : state.grant.eligible ? 'Eligible' : 'Ineligible';
  $('metric-credits').textContent = `Credits ${grant?.credits ?? '—'}`;
}
function renderNodes() {
  $('node-grid').innerHTML = state.observations.map((view, i) => {
    const observation = view?.observation;
    const ready = Boolean(observation?.ready && observation.round_id === state.grant?.state?.round_id);
    const error = observation?.error;
    const cardClass = ready ? 'ready' : error ? 'error' : '';
    const status = ready ? pill('READY','good') : error ? pill('ERROR','bad') : pill('Pending');
    const details = ready ? `Stable releases <b>${esc(observation.candidate?.stable_count ?? 0)}</b> · ${esc(observation.candidate?.releases?.length ?? 0)} records`
      : error ? esc(error.code) : 'No complete observation for the current round';
    return `<article class="node-card ${cardClass}"><div class="node-top"><strong>Node 0${i+1}</strong>${status}</div><div class="node-id" title="${esc(view?.node_id || '')}">${esc(view?.node_id || 'Not connected')}</div><p class="node-data">${details}</p><button class="button secondary node-refresh" data-node="${i}" type="button" ${state.busy ? 'disabled' : ''}>Refresh Node Observation</button></article>`;
  }).join('');
  document.querySelectorAll('.node-refresh').forEach(button => button.addEventListener('click', () => run(`Node ${Number(button.dataset.node)+1} observation`, async () => { dataOf(await read(Number(button.dataset.node), METHODS.refresh)); await refreshAll(); })));
}
function renderCandidate() {
  const candidate = state.candidate;
  $('candidate-status').className = `badge ${candidate ? 'good' : 'muted'}`;
  $('candidate-status').textContent = candidate ? `Round ${candidate.round_id}` : 'Awaiting Observation';
  if (!candidate) { $('candidate-content').className = 'empty'; $('candidate-content').textContent = 'Release records will appear after a node prepares a candidate.'; return; }
  const releases = candidate.releases || [];
  $('candidate-content').className = '';
  $('candidate-content').innerHTML = row('GitHub repository', `${candidate.config.repo_owner}/${candidate.config.repo_name}`)
    + row('Repository ID', candidate.config.repo_id) + row('Records in window', releases.length)
    + row('Stable releases / threshold', `${candidate.stable_count} / ${candidate.config.min_stable_releases}`)
    + `<div class="release-list">${releases.length ? releases.map(release => `<div class="release-row"><span>#${esc(release.id)}</span><span>${esc(new Date(Number(release.published_at)*1000).toLocaleString('en-US',{timeZone:'UTC'}))} UTC</span><span class="tag ${release.prerelease ? 'pre' : ''}">${release.prerelease ? 'Pre-release' : 'Stable'}</span></div>`).join('') : '<div class="member">No Releases in this window.</div>'}</div>`;
}
function renderCommittee() {
  const committee = state.committee;
  const active = Boolean(committee?.active);
  $('committee-status').className = `badge ${active ? 'good' : committee?.configured ? 'bad' : 'muted'}`;
  $('committee-status').textContent = active ? 'Active' : committee?.configured ? 'Pending activation' : 'Not configured';
  $('committee-content').className = committee ? '' : 'empty';
  $('committee-content').innerHTML = committee ? row('Threshold', `${committee.threshold} / ${committee.node_ids.length}`)
    + committee.node_ids.map(id => `<div class="member">${esc(id)}</div>`).join('') : 'Committee members and threshold will appear here.';
}
function renderSnapshot() {
  const grant = state.grant?.state;
  const snapshot = grant?.snapshot;
  $('claim-status').className = `badge ${grant?.claimed ? 'good' : snapshot && !state.grant?.eligible ? 'bad' : 'muted'}`;
  $('claim-status').textContent = grant?.claimed ? 'Claimed' : !snapshot ? 'Not certified' : state.grant.eligible ? 'Ready to claim' : 'Not eligible to claim';
  $('snapshot-content').className = snapshot ? '' : 'empty';
  $('snapshot-content').innerHTML = snapshot ? row('Stable releases', snapshot.candidate.stable_count)
    + row('Beneficiary', short(snapshot.candidate.config.beneficiary, 14, 10))
    + row('Eligibility', state.grant.eligible ? 'Grant criteria met' : 'Grant criteria not met')
    + row('Credits awarded', grant.credits)
    + `<div class="release-list">${snapshot.signer_node_ids.map(id => `<div class="member">${esc(id)}</div>`).join('')}</div>`
    : 'The final certified result is stored in Network State.';
}
function render() { renderMetrics(); renderNodes(); renderCandidate(); renderCommittee(); renderSnapshot(); }
async function refreshAll() {
  saveConfig();
  const {rpcs} = config();
  const results = await Promise.allSettled([
    read(0, METHODS.getGrant), read(0, METHODS.committee),
    ...rpcs.map((_, i) => read(i, METHODS.observation)),
  ]);
  state.grant = results[0].status === 'fulfilled' && results[0].value.ok ? results[0].value.data : null;
  state.committee = results[1].status === 'fulfilled' && results[1].value.ok ? results[1].value.data : null;
  state.observations = results.slice(2).map(result => result.status === 'fulfilled' && result.value.ok ? result.value.data : null);
  const readyIndex = state.observations.findIndex(view => view?.observation?.ready && view.observation.round_id === state.grant?.state?.round_id);
  state.candidate = null;
  if (readyIndex !== -1) {
    try { state.candidate = dataOf(await read(readyIndex, METHODS.candidate)); } catch (_) { /* candidate panel keeps empty state */ }
  }
  render();
  const online = Boolean(state.grant);
  $('network-pill').className = `pill ${online ? 'online' : 'offline'}`;
  $('network-pill').innerHTML = `<span class="dot"></span>${online ? 'Nodes connected' : 'Connection failed'}`;
  if (!online) throw results[0].status === 'rejected' ? results[0].reason : new Error(results[0].value.error?.message || 'Could not read contract state');
  const failures = results.slice(1).filter(result => result.status === 'rejected' || !result.value.ok).length;
  if (failures) log(`${failures} node or API reads failed; check RPC URLs and deployment`, true);
}
async function observeAll() {
  for (let i = 0; i < 4; i++) {
    try { const view = dataOf(await read(i, METHODS.refresh)); log(`Node ${i+1} observation ready: ${view.observation?.candidate?.stable_count ?? 0} stable releases`); }
    catch (error) { log(`Node ${i+1} observation failed: ${error.message}`, true); }
  }
  await refreshAll();
}
async function waitFor(predicate, label, timeoutMs = 60000) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      await refreshAll();
      if (predicate()) return;
    } catch (error) { lastError = error; }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  throw new Error(`${label} was not confirmed within 60 seconds${lastError ? `: ${lastError.message}` : '; check shared state before retrying'}`);
}
async function certify() {
  const submission = dataOf(await read(0, METHODS.certify));
  if (!submission.submitted) throw new Error('Certification was not submitted');
  log(`Certification submitted: ${submission.grant_id} / round ${submission.round_id}`);
  await waitFor(() => state.grant?.state?.snapshot?.candidate?.round_id === submission.round_id, 'certified snapshot');
}
async function connectWallet() {
  if (state.local?.demo_signing) { log('Local demo signing uses separate administrator and beneficiary devnet accounts.'); return; }
  if (!window.ethereum) throw new Error('No browser wallet detected. Install an EIP-1193 wallet such as MetaMask.');
  const accounts = await window.ethereum.request({method:'eth_requestAccounts'});
  if (!accounts?.length) throw new Error('The wallet did not provide an account');
  state.account = accounts[0];
  $('wallet-button').textContent = short(state.account, 8, 6);
  log(`Wallet connected: ${state.account}`);
}
async function ensureWalletNetwork() {
  if (!state.account) await connectWallet();
  const {rpcs} = config();
  const chainId = await rpc(rpcs[0], 'eth_chainId', []);
  const current = await window.ethereum.request({method:'eth_chainId'});
  if (String(current).toLowerCase() !== String(chainId).toLowerCase()) {
    try { await window.ethereum.request({method:'wallet_switchEthereumChain',params:[{chainId}]}); }
    catch (error) {
      if (error.code !== 4902) throw error;
      await window.ethereum.request({method:'wallet_addEthereumChain',params:[{chainId, chainName:'GrantsTrace local Lyquor', rpcUrls:[rpcs[0]], nativeCurrency:{name:'Ether',symbol:'ETH',decimals:18}}]});
    }
  }
}
async function send(selector, args = []) {
  if (state.local?.demo_signing) {
    const current = config();
    if (current.rpcs.some((rpc, i) => rpc !== DEFAULT_RPCS[i])) throw new Error('Local demo signing only supports the four default RPC ports');
    const actions = {
      [METHODS.configureGrant]: 'configureGrant', [METHODS.configureCommittee]: 'configureCommittee',
      [METHODS.startRound]: 'startRound', [METHODS.claim]: 'claim',
      [METHODS.advanceEpoch]: 'advanceEpoch', [METHODS.finalizeEpoch]: 'finalizeEpoch',
    };
    const action = actions[selector];
    if (!action) throw new Error('Unsupported local signing action');
    const payload = {action, address: current.address};
    if (action === 'configureGrant' || action === 'configureCommittee') payload.value = args[0].value;
    if (action === 'advanceEpoch' || action === 'finalizeEpoch') payload.target = args[1].value;
    const result = await localPost('/local/send', payload);
    log(result.hash ? `Local test transaction sent: ${short(result.hash, 14, 10)}; waiting for business state confirmation`
      : `Committee epoch action submitted; waiting for node confirmation`);
    return result.hash || result.submitted;
  }
  await ensureWalletNetwork();
  const {address} = config();
  const hash = await window.ethereum.request({method:'eth_sendTransaction',params:[{from:state.account,to:address,data:encodeCall(selector,args)}]});
  log(`Transaction sent: ${short(hash, 14, 10)}; waiting for business state confirmation`);
  return hash;
}
function stringArg(value) { return {type:'string',value}; }
async function configureGrant() {
  const raw = $('grant-config').value.trim();
  const grant = JSON.parse(raw);
  if (!grant || Array.isArray(grant) || typeof grant !== 'object') throw new Error('GrantConfig must be a JSON object');
  if (state.grant?.state?.config) throw new Error('This deployment already has a grant configuration');
  await send(METHODS.configureGrant, [stringArg(raw)]);
  await waitFor(() => Boolean(state.grant?.state?.config), 'grant configuration');
}
async function configureCommittee() {
  const ids = JSON.parse($('node-ids').value);
  if (!Array.isArray(ids) || ids.length !== 4 || new Set(ids).size !== 4 || ids.some(id => typeof id !== 'string')) throw new Error('Provide four distinct Node IDs');
  if (state.committee?.configured) throw new Error('The committee is already configured');
  await send(METHODS.configureCommittee, [stringArg(JSON.stringify({node_ids:ids}))]);
  await waitFor(() => Boolean(state.committee?.configured), 'committee configuration');
}
async function activateCommittee() {
  const target = $('lyquid-target').value.trim();
  if (!/^0x[0-9a-fA-F]{40}$/.test(target)) throw new Error('Enter the output of shaker to-hex LYQUID_ID');
  if (!state.committee?.configured) throw new Error('Configure the committee first');
  await send(METHODS.advanceEpoch, [stringArg('releases'),{type:'address',value:target},{type:'bool',value:false}]);
  await send(METHODS.finalizeEpoch, [stringArg('releases'),{type:'address',value:target},{type:'bool',value:false}]);
  await waitFor(() => Boolean(state.committee?.active), 'committee activation');
}
async function startRound() {
  const previous = Number(state.grant?.state?.round_id ?? 0);
  if (!state.grant?.state?.config) throw new Error('Configure the grant first');
  if (!state.committee?.active) throw new Error('Activate the four-node committee first');
  await send(METHODS.startRound);
  await waitFor(() => Number(state.grant?.state?.round_id ?? 0) > previous, 'new round');
}
async function claim() {
  if (!state.grant?.eligible) throw new Error('The current snapshot does not meet the claim criteria');
  if (state.grant.state.claimed) throw new Error('Credits have already been claimed');
  const beneficiary = state.grant.state.config?.beneficiary;
  if (state.local?.demo_signing) {
    if (state.local.beneficiary !== beneficiary?.toLowerCase()) throw new Error('The configured beneficiary is not the local demo account');
  } else {
    await ensureWalletNetwork();
    if (state.account.toLowerCase() !== beneficiary?.toLowerCase()) throw new Error('The connected wallet is not the designated beneficiary');
  }
  await send(METHODS.claim);
  await waitFor(() => Boolean(state.grant?.state?.claimed), 'credit claim');
}
async function localPost(path, payload) {
  const response = await fetch(path, {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify(payload)});
  const result = await response.json();
  if (!response.ok || !result.ok) throw new Error(result.error || `Local server HTTP ${response.status}`);
  return result.data;
}
async function bootstrapLocal() {
  try {
    const response = await fetch('/local/status');
    if (!response.ok) return;
    const result = await response.json();
    if (!result.ok) return;
    state.local = result;
    $('node-ids').value = JSON.stringify(result.node_ids, null, 2);
    if (result.last_deploy && $('contract-address').value === DEFAULT_ADDRESS) {
      $('contract-address').value = result.last_deploy.contract;
      $('lyquid-target').value = result.last_deploy.target;
      saveConfig();
    }
    if (result.demo_signing) {
      $('wallet-button').textContent = 'Local test accounts';
      const current = JSON.parse($('grant-config').value);
      current.beneficiary = result.beneficiary;
      $('grant-config').value = JSON.stringify(current, null, 2);
      log('Local demo signing is ready; private keys remain in the local server process');
    }
  } catch (_) { /* The static page also works with a browser wallet. */ }
}
async function deployLocal() {
  if (!state.local?.demo_signing) throw new Error('Start the local demo server with scripts/frontend.sh');
  const result = await localPost('/local/deploy', {});
  $('contract-address').value = result.contract;
  $('lyquid-target').value = result.target;
  saveConfig();
  log(`New deployment ${result.lyquid_id}; contract ${result.contract}`);
  await waitFor(() => Boolean(state.grant?.state), 'new deployment');
}
async function runAcceptance() {
  if (!state.local?.demo_signing) throw new Error('Full acceptance testing requires the local demo server');
  const configValue = JSON.parse($('grant-config').value);
  const ids = JSON.parse($('node-ids').value);
  await localPost('/local/acceptance/start', {config: configValue, node_ids: ids});
  $('acceptance-output').textContent = 'Acceptance test started; deploying a new Lyquid…';
  const deadline = Date.now() + 12 * 60 * 1000;
  while (Date.now() < deadline) {
    const result = await fetch('/local/acceptance/status').then(response => response.json());
    $('acceptance-output').textContent = result.lines.join('\n') || 'Acceptance test is running…';
    $('acceptance-output').scrollTop = $('acceptance-output').scrollHeight;
    if (result.done) {
      if (result.exit_code !== 0) throw new Error('Acceptance test failed; see the output above');
      await refreshAll();
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  throw new Error('Acceptance test exceeded 12 minutes; check the local server');
}
function bind() {
  $('wallet-button').addEventListener('click', () => run('Connect Wallet', connectWallet));
  $('refresh-all-button').addEventListener('click', () => run('Refresh All Status', refreshAll));
  $('deploy-button').addEventListener('click', () => run('New Demo Deployment', deployLocal));
  $('acceptance-button').addEventListener('click', () => run('Full Four-Node Acceptance Test', runAcceptance));
  $('observe-all-button').addEventListener('click', () => run('Four-Node Observation', observeAll));
  $('certify-button').addEventListener('click', () => run('Certification', certify));
  $('claim-button').addEventListener('click', () => run('Claim', claim));
  $('configure-grant-button').addEventListener('click', () => run('Configure Grant', configureGrant));
  $('configure-committee-button').addEventListener('click', () => run('Configure Committee', configureCommittee));
  $('activate-committee-button').addEventListener('click', () => run('Activate Committee', activateCommittee));
  $('start-round-button').addEventListener('click', () => run('Start Round', startRound));
  $('clear-log-button').addEventListener('click', () => $('activity-log').replaceChildren());
  document.querySelectorAll('.settings input').forEach(input => input.addEventListener('change', saveConfig));
  window.ethereum?.on?.('accountsChanged', accounts => {
    state.account = accounts?.[0] || null;
    $('wallet-button').textContent = state.account ? short(state.account, 8, 6) : 'Connect Wallet';
  });
}
loadConfig(); bind(); render();
bootstrapLocal().then(() => run('Initial Connection', refreshAll));
