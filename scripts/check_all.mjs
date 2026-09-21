import fs from 'fs';

const data = JSON.parse(fs.readFileSync('subscription_dump.json', 'utf8'));
data.forEach((item, idx) => {
  const proxies = (item.outbounds || []).filter(o => !['freedom', 'blackhole', 'dns'].includes(o.protocol));
  console.log(`[${idx}] remarks: '${item.remarks}' -> ${proxies.length} proxies (balancers: ${item.routing?.balancers ? JSON.stringify(item.routing.balancers) : 'no'})`);
});

