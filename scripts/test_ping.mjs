import fs from 'fs';
import net from 'net';

const data = JSON.parse(fs.readFileSync('subscription_dump.json', 'utf8'));

function pingTcp(host, port) {
  return new Promise((resolve) => {
    const start = Date.now();
    const socket = new net.Socket();
    socket.setTimeout(2500);
    socket.on('connect', () => {
      const elapsed = Date.now() - start;
      socket.destroy();
      resolve(elapsed);
    });
    socket.on('timeout', () => {
      socket.destroy();
      resolve('timeout');
    });
    socket.on('error', () => {
      socket.destroy();
      resolve('timeout');
    });
    socket.connect(port, host);
  });
}

async function testAll() {
  const start = Date.now();
  console.log('Testing TCP ping for all 33 servers...');
  const promises = data.map(async (item, idx) => {
    const ob = (item.outbounds || []).find(o => !['freedom', 'blackhole', 'dns'].includes(o.protocol));
    const addr = ob?.settings?.vnext?.[0]?.address || ob?.settings?.servers?.[0]?.address;
    const port = ob?.settings?.vnext?.[0]?.port || ob?.settings?.servers?.[0]?.port;
    if (!addr || !port) return;
    const res = await pingTcp(addr, port);
    console.log(`[${idx}] ${item.remarks} -> ${res} ms (${addr}:${port})`);
  });
  await Promise.all(promises);
  console.log('Total time for ALL servers:', Date.now() - start, 'ms');
}

testAll();

