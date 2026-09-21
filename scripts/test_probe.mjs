import fs from 'fs';
import { spawn } from 'child_process';
import { SocksProxyAgent } from 'socks-proxy-agent';
import fetch from 'node-fetch';

async function run() {
  const data = JSON.parse(fs.readFileSync('subscription_dump.json', 'utf8'));
  console.log('Testing servers 0 (Auto), 1 (FI), 2 (US)');
  // We can test if xray starts with 3 socks inbounds and routes each to corresponding outbound
}
run();

