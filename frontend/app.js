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
const state = { grant: null, committee: null, observations: [null, null, null, null], candidate: null, account: null, busy: false };

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
function short(value, start = 10, end = 6) {
  const s = String(value ?? '');
  return s.length > start + end + 3 ? `${s.slice(0,start)}…${s.slice(-end)}` : s;
}
function config() {
  const address = $('contract-address').value.trim();
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) throw new Error('请填写有效的 Lyquid 合约地址');
  const rpcs = DEFAULT_RPCS.map((_, i) => $('rpc-' + i).value.trim());
  for (const rpc of rpcs) {
    const url = new URL(rpc);
    if (!['http:', 'https:'].includes(url.protocol)) throw new Error('RPC 地址必须使用 HTTP 或 HTTPS');
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
  li.textContent = `${new Date().toLocaleTimeString('zh-CN', {hour12:false})}  ${message}`;
  if (fail) li.className = 'fail';
  $('activity-log').prepend(li);
}
function setBusy(busy) {
  state.busy = busy;
  document.querySelectorAll('button').forEach(button => {
    if (button.id !== 'clear-log-button') button.disabled = busy;
  });
}
async function run(label, task) {
  if (state.busy) return;
  setBusy(true);
  log(`${label}开始`);
  try { await task(); log(`${label}完成`); }
  catch (error) { log(`${label}失败：${error.message}`, true); }
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
  if (!/^0x[0-9a-fA-F]{40}$/.test(value)) throw new Error('LVM 目标地址无效');
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
  throw new Error('不支持的 ABI 参数');
}
function decodeString(hex) {
  const data = strip0x(hex || '');
  if (data.length < 128) throw new Error('RPC 返回值不是 ABI string');
  const offset = Number(BigInt('0x' + data.slice(0, 64))) * 2;
  if (offset + 64 > data.length) throw new Error('RPC string 偏移量无效');
  const length = Number(BigInt('0x' + data.slice(offset, offset + 64)));
  if (!Number.isSafeInteger(length) || length > 1_000_000 || offset + 64 + length * 2 > data.length) throw new Error('RPC string 长度无效');
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
  if (result.error) throw new Error(result.error.message || `RPC 错误 ${result.error.code}`);
  return result.result;
}
async function read(index, selector) {
  const {address, rpcs} = config();
  const result = await rpc(rpcs[index], 'eth_call', [{to:address, data:selector}, 'latest']);
  const envelope = parseJsonLossless(decodeString(result));
  if (!envelope || typeof envelope.ok !== 'boolean') throw new Error('接口返回格式无效');
  return envelope;
}
function dataOf(envelope) {
  if (!envelope.ok) throw new Error(`${envelope.error?.code || 'UNKNOWN'}：${envelope.error?.message || '操作失败'}`);
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
  $('metric-eligible').textContent = state.grant ? (state.grant.eligible ? '已满足' : '未满足') : '—';
  $('metric-credits').textContent = `积分 ${grant?.credits ?? '—'}`;
}
function renderNodes() {
  $('node-grid').innerHTML = state.observations.map((view, i) => {
    const observation = view?.observation;
    const ready = Boolean(observation?.ready && observation.round_id === state.grant?.state?.round_id);
    const error = observation?.error;
    const cardClass = ready ? 'ready' : error ? 'error' : '';
    const status = ready ? pill('READY','good') : error ? pill('ERROR','bad') : pill('等待中');
    const details = ready ? `正式发布 <b>${esc(observation.candidate?.stable_count ?? 0)}</b> · ${esc(observation.candidate?.releases?.length ?? 0)} 条记录`
      : error ? esc(error.code) : '尚无当前轮次的完整观察';
    return `<article class="node-card ${cardClass}"><div class="node-top"><strong>节点 0${i+1}</strong>${status}</div><div class="node-id" title="${esc(view?.node_id || '')}">${esc(view?.node_id || '尚未连接')}</div><p class="node-data">${details}</p><button class="button secondary node-refresh" data-node="${i}" type="button" ${state.busy ? 'disabled' : ''}>刷新该节点观察</button></article>`;
  }).join('');
  document.querySelectorAll('.node-refresh').forEach(button => button.addEventListener('click', () => run(`节点 ${Number(button.dataset.node)+1} 观察`, async () => { dataOf(await read(Number(button.dataset.node), METHODS.refresh)); await refreshAll(); })));
}
function renderCandidate() {
  const candidate = state.candidate;
  $('candidate-status').className = `badge ${candidate ? 'good' : 'muted'}`;
  $('candidate-status').textContent = candidate ? `Round ${candidate.round_id}` : '等待观察';
  if (!candidate) { $('candidate-content').className = 'empty'; $('candidate-content').textContent = '至少一个节点的候选准备好后，这里会显示参与认证的 Release 记录。'; return; }
  const releases = candidate.releases || [];
  $('candidate-content').className = '';
  $('candidate-content').innerHTML = row('GitHub 仓库', `${candidate.config.repo_owner}/${candidate.config.repo_name}`)
    + row('仓库数字 ID', candidate.config.repo_id) + row('窗口内记录', releases.length)
    + row('正式版本 / 门槛', `${candidate.stable_count} / ${candidate.config.min_stable_releases}`)
    + `<div class="release-list">${releases.length ? releases.map(release => `<div class="release-row"><span>#${esc(release.id)}</span><span>${esc(new Date(Number(release.published_at)*1000).toLocaleString('zh-CN',{timeZone:'UTC'}))} UTC</span><span class="tag ${release.prerelease ? 'pre' : ''}">${release.prerelease ? '预发布' : '正式版'}</span></div>`).join('') : '<div class="member">该窗口没有 Release 记录。</div>'}</div>`;
}
function renderCommittee() {
  const committee = state.committee;
  const active = Boolean(committee?.active);
  $('committee-status').className = `badge ${active ? 'good' : committee?.configured ? 'bad' : 'muted'}`;
  $('committee-status').textContent = active ? '已激活' : committee?.configured ? '待激活' : '未配置';
  $('committee-content').className = committee ? '' : 'empty';
  $('committee-content').innerHTML = committee ? row('阈值', `${committee.threshold} / ${committee.node_ids.length}`)
    + committee.node_ids.map(id => `<div class="member">${esc(id)}</div>`).join('') : '读取委员会状态后显示成员与阈值。';
}
function renderSnapshot() {
  const grant = state.grant?.state;
  const snapshot = grant?.snapshot;
  $('claim-status').className = `badge ${grant?.claimed ? 'good' : snapshot ? 'muted' : 'muted'}`;
  $('claim-status').textContent = grant?.claimed ? '已领取' : snapshot ? '待领取' : '未认证';
  $('snapshot-content').className = snapshot ? '' : 'empty';
  $('snapshot-content').innerHTML = snapshot ? row('正式发布数', snapshot.candidate.stable_count)
    + row('受益人', short(snapshot.candidate.config.beneficiary, 14, 10))
    + row('资格', state.grant.eligible ? '满足资助条件' : '未满足条件')
    + row('已登记积分', grant.credits)
    + `<div class="release-list">${snapshot.signer_node_ids.map(id => `<div class="member">${esc(id)}</div>`).join('')}</div>`
    : '最终认证结果会保存在 Network State。';
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
  $('network-pill').innerHTML = `<span class="dot"></span>${online ? '节点已连接' : '无法连接'}`;
  if (!online) throw results[0].status === 'rejected' ? results[0].reason : new Error(results[0].value.error?.message || '合约状态读取失败');
  const failures = results.slice(1).filter(result => result.status === 'rejected' || !result.value.ok).length;
  if (failures) log(`${failures} 个节点或接口读取失败；请检查 RPC 地址与部署`, true);
}
async function observeAll() {
  for (let i = 0; i < 4; i++) {
    try { const view = dataOf(await read(i, METHODS.refresh)); log(`节点 ${i+1} 观察成功：${view.observation?.candidate?.stable_count ?? 0} 个正式版本`); }
    catch (error) { log(`节点 ${i+1} 观察失败：${error.message}`, true); }
  }
  await refreshAll();
}
async function waitFor(predicate, label, timeoutMs = 60000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    await refreshAll();
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  throw new Error(`${label}在 60 秒内未被状态确认，请先检查共享状态再重试`);
}
async function certify() {
  const submission = dataOf(await read(0, METHODS.certify));
  if (!submission.submitted) throw new Error('认证没有提交成功');
  log(`认证已提交：${submission.grant_id} / round ${submission.round_id}`);
  await waitFor(() => state.grant?.state?.snapshot?.candidate?.round_id === submission.round_id, '认证快照');
}
async function connectWallet() {
  if (!window.ethereum) throw new Error('未检测到浏览器钱包；请安装 MetaMask 等 EIP-1193 钱包');
  const accounts = await window.ethereum.request({method:'eth_requestAccounts'});
  if (!accounts?.length) throw new Error('钱包没有提供账户');
  state.account = accounts[0];
  $('wallet-button').textContent = short(state.account, 8, 6);
  log(`钱包已连接：${state.account}`);
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
  await ensureWalletNetwork();
  const {address} = config();
  const hash = await window.ethereum.request({method:'eth_sendTransaction',params:[{from:state.account,to:address,data:encodeCall(selector,args)}]});
  log(`交易已发送：${short(hash, 14, 10)}。等待业务状态确认`);
  return hash;
}
function stringArg(value) { return {type:'string',value}; }
async function configureGrant() {
  const raw = $('grant-config').value.trim();
  const grant = JSON.parse(raw);
  if (!grant || Array.isArray(grant) || typeof grant !== 'object') throw new Error('GrantConfig 必须是 JSON 对象');
  if (state.grant?.state?.config) throw new Error('此部署已配置资助项目');
  await send(METHODS.configureGrant, [stringArg(raw)]);
  await waitFor(() => Boolean(state.grant?.state?.config), '资助配置');
}
async function configureCommittee() {
  const ids = JSON.parse($('node-ids').value);
  if (!Array.isArray(ids) || ids.length !== 4 || new Set(ids).size !== 4 || ids.some(id => typeof id !== 'string')) throw new Error('必须提供四个不同的 Node ID');
  if (state.committee?.configured) throw new Error('委员会已经配置');
  await send(METHODS.configureCommittee, [stringArg(JSON.stringify({node_ids:ids}))]);
  await waitFor(() => Boolean(state.committee?.configured), '委员会配置');
}
async function activateCommittee() {
  const target = $('lyquid-target').value.trim();
  if (!/^0x[0-9a-fA-F]{40}$/.test(target)) throw new Error('请填写 shaker to-hex LYQUID_ID 的结果');
  if (!state.committee?.configured) throw new Error('请先配置委员会');
  await send(METHODS.advanceEpoch, [stringArg('releases'),{type:'address',value:target},{type:'bool',value:false}]);
  await send(METHODS.finalizeEpoch, [stringArg('releases'),{type:'address',value:target},{type:'bool',value:false}]);
  await waitFor(() => Boolean(state.committee?.active), '委员会激活');
}
async function startRound() {
  const previous = Number(state.grant?.state?.round_id ?? 0);
  if (!state.grant?.state?.config) throw new Error('请先配置资助项目');
  await send(METHODS.startRound);
  await waitFor(() => Number(state.grant?.state?.round_id ?? 0) > previous, '新轮次');
}
async function claim() {
  if (!state.grant?.eligible) throw new Error('当前快照尚未满足领取条件');
  if (state.grant.state.claimed) throw new Error('积分已领取');
  const beneficiary = state.grant.state.config?.beneficiary;
  await ensureWalletNetwork();
  if (state.account.toLowerCase() !== beneficiary?.toLowerCase()) throw new Error('当前钱包不是固定受益账户');
  await send(METHODS.claim);
  await waitFor(() => Boolean(state.grant?.state?.claimed), '积分领取');
}
function bind() {
  $('wallet-button').addEventListener('click', () => run('连接钱包', connectWallet));
  $('refresh-all-button').addEventListener('click', () => run('刷新全部状态', refreshAll));
  $('observe-all-button').addEventListener('click', () => run('四节点观察', observeAll));
  $('certify-button').addEventListener('click', () => run('认证', certify));
  $('claim-button').addEventListener('click', () => run('领取', claim));
  $('configure-grant-button').addEventListener('click', () => run('配置资助', configureGrant));
  $('configure-committee-button').addEventListener('click', () => run('配置委员会', configureCommittee));
  $('activate-committee-button').addEventListener('click', () => run('激活委员会', activateCommittee));
  $('start-round-button').addEventListener('click', () => run('开启轮次', startRound));
  $('clear-log-button').addEventListener('click', () => $('activity-log').replaceChildren());
  document.querySelectorAll('.settings input').forEach(input => input.addEventListener('change', saveConfig));
  window.ethereum?.on?.('accountsChanged', accounts => {
    state.account = accounts?.[0] || null;
    $('wallet-button').textContent = state.account ? short(state.account, 8, 6) : '连接钱包';
  });
}
loadConfig(); bind(); render();
run('初始连接', refreshAll);
