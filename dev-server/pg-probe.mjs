// 纯 Node 的 PostgreSQL 最小客户端：只实现「连接认证 + 简单查询协议」，够数据源探测用。
// 支持 trust / cleartext / md5 / SCRAM-SHA-256 四种认证；不支持 SSL（需要时明确报错）。
// 为什么不用 pg 包：app/node_modules 没装，沙箱也装不了；探测就三条 SQL，不值得引依赖。
import net from 'node:net';
import crypto from 'node:crypto';

const PROTOCOL_3_0 = 196608;

export class PgConn {
  /**
   * @param {{host?:string,port?:number|string,user?:string,password?:string,database?:string}} conf
   */
  constructor(conf) {
    this.host = conf.host || '127.0.0.1';
    this.port = Number(conf.port) || 5432;
    this.user = conf.user || 'postgres';
    this.password = conf.password || '';
    this.database = conf.database || this.user;
    this.buf = Buffer.alloc(0);
    this.waiter = null; // { resolve, reject } 等待一次查询收尾（Z）
    this.readyResolve = null;
    this.readyReject = null;
    this.columns = [];
    this.rows = [];
    this.closed = false;
  }

  /** 建连 + 认证，成功后处于 ReadyForQuery。 */
  connect(timeoutMs = 8000) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this._settleReady(reject, new Error(`连接超时（${timeoutMs}ms）：${this.host}:${this.port}`));
      }, timeoutMs);
      this._timer = timer;
      this.socket = net.connect({ host: this.host, port: this.port });
      this.socket.setTimeout(timeoutMs);
      this.socket.on('error', (e) => this._settleReady(reject, new Error(`连接失败：${e.message}`)));
      this.socket.on('timeout', () => this._settleReady(reject, new Error(`连接超时（${timeoutMs}ms）：${this.host}:${this.port}`)));
      this.socket.on('close', () => {
        if (this.waiter) {
          const w = this.waiter;
          this.waiter = null;
          w.reject(new Error('连接被服务端关闭'));
        }
      });
      this.socket.on('data', (d) => {
        this.buf = Buffer.concat([this.buf, d]);
        this._pump();
      });
      this.socket.on('connect', () => {
        // StartupMessage：协议 3.0，只带 user/database
        const params = Buffer.from(`user\0${this.user}\0database\0${this.database}\0\0`, 'utf8');
        const head = Buffer.alloc(8);
        head.writeInt32BE(8 + params.length, 0);
        head.writeInt32BE(PROTOCOL_3_0, 4);
        this.socket.write(Buffer.concat([head, params]));
      });
      this.readyResolve = resolve;
      this.readyReject = reject;
    });
  }

  /** 简单查询：返回 { columns: string[], rows: (string|null)[][] }（文本格式值）。 */
  query(sql, timeoutMs = 15000) {
    return new Promise((resolve, reject) => {
      this.columns = [];
      this.rows = [];
      let timer = null;
      this.waiter = {
        resolve: (v) => {
          if (timer) clearTimeout(timer);
          resolve(v);
        },
        reject: (e) => {
          if (timer) clearTimeout(timer);
          reject(e);
        },
      };
      timer = setTimeout(() => {
        if (this.waiter) {
          const w = this.waiter;
          this.waiter = null;
          w.reject(new Error(`查询超时（${timeoutMs}ms）`));
        }
      }, timeoutMs);
      this._send('Q', Buffer.from(sql + '\0', 'utf8'));
    });
  }

  end() {
    this.closed = true;
    try {
      this._send('X', Buffer.alloc(0));
    } catch {
      /* socket 可能已死，忽略 */
    }
    try {
      this.socket?.destroy();
    } catch {
      /* 同上 */
    }
    if (this._timer) clearTimeout(this._timer);
  }

  // ---- 内部 ----

  _send(type, payload) {
    const head = Buffer.alloc(5);
    head.write(type, 0, 'ascii');
    head.writeInt32BE(4 + payload.length, 1);
    this.socket.write(Buffer.concat([head, payload]));
  }

  _pump() {
    while (this.buf.length >= 5) {
      const type = String.fromCharCode(this.buf[0]);
      const len = this.buf.readInt32BE(1);
      if (len < 4 || this.buf.length < 1 + len) break;
      const payload = this.buf.subarray(5, 1 + len);
      this.buf = this.buf.subarray(1 + len);
      try {
        this._handle(type, payload);
      } catch (e) {
        this._settleReady(this.readyReject, e);
        this._settleWaiter(e);
      }
      if (this.closed) return;
    }
  }

  _handle(type, p) {
    switch (type) {
      case 'R':
        this._auth(p);
        return;
      case 'S':
      case 'K':
      case 'N':
        return; // ParameterStatus / BackendKeyData / Notice：忽略
      case 'Z': // ReadyForQuery：一次查询收尾 / 连接就绪
        if (this.waiter) {
          const w = this.waiter;
          this.waiter = null;
          w.resolve({ columns: this.columns.map((c) => c.name), raw: this.columns, rows: this.rows });
        } else if (this.readyResolve) {
          const f = this.readyResolve;
          this.readyResolve = null;
          this.readyReject = null;
          if (this._timer) clearTimeout(this._timer);
          f(this);
        }
        return;
      case 'T':
        this._rowDescription(p);
        return;
      case 'D':
        this._dataRow(p);
        return;
      case 'C':
        return; // CommandComplete
      case 'E': {
        const msg = this._parseError(p);
        const err = new Error(msg);
        if (this.waiter) {
          this._settleWaiter(err);
        } else {
          this._settleReady(this.readyReject, new Error(`认证/握手失败：${msg}`));
        }
        return;
      }
      default:
        return; // 未知消息类型：忽略（探测不需要完整协议）
    }
  }

  _settleWaiter(err) {
    if (this.waiter) {
      const w = this.waiter;
      this.waiter = null;
      if (err) w.reject(err);
    }
  }

  _settleReady(reject, err) {
    if (this.readyReject) {
      const r = this.readyReject;
      this.readyResolve = null;
      this.readyReject = null;
      if (this._timer) clearTimeout(this._timer);
      r(err);
    }
    try {
      this.socket?.destroy();
    } catch {
      /* 忽略 */
    }
  }

  _auth(p) {
    const code = p.readInt32BE(0);
    if (code === 0) return; // AuthenticationOk，等 Z
    if (code === 3) {
      // 明文口令
      this._send('p', Buffer.from(this.password + '\0', 'utf8'));
      return;
    }
    if (code === 5) {
      // MD5：md5(md5(password+user) + salt)
      const salt = p.subarray(4, 8);
      const inner = crypto.createHash('md5').update(this.password + this.user, 'utf8').digest('hex');
      const outer = crypto.createHash('md5').update(inner, 'utf8').update(salt).digest('hex');
      this._send('p', Buffer.from(`md5${outer}\0`, 'utf8'));
      return;
    }
    if (code === 10) {
      this._scramStart();
      return;
    }
    if (code === 11) {
      this._scramServerFirst(p);
      return;
    }
    if (code === 12) return; // SASLFinal：服务端签名校验从略（不影响查询）
    this._settleReady(this.readyReject, new Error(`不支持的认证方式（code=${code}）`));
  }

  _scramStart() {
    this.scramNonce = crypto.randomBytes(18).toString('base64');
    this.scramClientFirstBare = `n=,r=${this.scramNonce}`;
    const mech = Buffer.from('SCRAM-SHA-256\0', 'utf8');
    const bare = Buffer.from(`n,,${this.scramClientFirstBare}`, 'utf8');
    const len = Buffer.alloc(4);
    len.writeInt32BE(bare.length, 0);
    this._send('p', Buffer.concat([mech, len, bare]));
  }

  _scramServerFirst(p) {
    const msg = p.subarray(4).toString('utf8'); // r=..,s=..,i=..
    const kv = {};
    for (const part of msg.split(',')) kv[part[0]] = part.slice(2);
    const serverNonce = kv.r || '';
    const salt = Buffer.from(kv.s || '', 'base64');
    const iters = parseInt(kv.i || '4096', 10);
    if (!serverNonce.startsWith(this.scramNonce || '')) {
      this._settleReady(this.readyReject, new Error('SCRAM nonce 不匹配'));
      return;
    }
    const salted = crypto.pbkdf2Sync(this.password, salt, iters, 32, 'sha256');
    const clientKey = crypto.createHmac('sha256', salted).update('Client Key').digest();
    const storedKey = crypto.createHash('sha256').update(clientKey).digest();
    const clientFinalBare = `c=biws,r=${serverNonce}`;
    const authMessage = `${this.scramClientFirstBare},${msg},${clientFinalBare}`;
    const clientSig = crypto.createHmac('sha256', storedKey).update(authMessage, 'utf8').digest();
    const proof = Buffer.alloc(32);
    for (let i = 0; i < 32; i++) proof[i] = clientKey[i] ^ clientSig[i];
    this._send('p', Buffer.from(`${clientFinalBare},p=${proof.toString('base64')}`, 'utf8'));
  }

  _rowDescription(p) {
    const n = p.readInt16BE(0);
    let off = 2;
    this.columns = [];
    for (let i = 0; i < n; i++) {
      let end = off;
      while (end < p.length && p[end] !== 0) end++;
      const name = p.subarray(off, end).toString('utf8');
      off = end + 1 + 4 + 2 + 4 + 2 + 4 + 2; // name\0 + tableOID + attr + typeOID + typlen + typmod + format
      this.columns.push({ name });
    }
  }

  _dataRow(p) {
    const n = p.readInt16BE(0);
    let off = 2;
    const row = [];
    for (let i = 0; i < n; i++) {
      const len = p.readInt32BE(off);
      off += 4;
      if (len === -1) {
        row.push(null);
      } else {
        row.push(p.subarray(off, off + len).toString('utf8'));
        off += len;
      }
    }
    this.rows.push(row);
  }

  _parseError(p) {
    // 字段格式：code(1字节) + cstring，直到 \0
    const fields = {};
    let off = 0;
    while (off < p.length) {
      const code = String.fromCharCode(p[off]);
      if (code === '\0') break;
      let end = off + 1;
      while (end < p.length && p[end] !== 0) end++;
      fields[code] = p.subarray(off + 1, end).toString('utf8');
      off = end + 1;
    }
    return `${fields.S || 'ERROR'}: ${fields.M || '未知服务端错误'}${fields.C ? ` (SQLSTATE ${fields.C})` : ''}`;
  }
}
