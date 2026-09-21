const fs = require('fs');

async function main() {
  let txt;
  if (fs.existsSync('subscription_dump.json')) {
    txt = fs.readFileSync('subscription_dump.json', 'utf8');
  } else {
    const res = await fetch('https://whitelist.blessedvpn.space/subs/e42ff82e-e5bd-48f8-847a-80d8cc08c750/235a5a4a-ca3c-481c-a28b-51e9038f296e', {
      headers: { 'User-Agent': 'Lumen/0.1 Xray/26.9.9' }
    });
    txt = await res.text();
    fs.writeFileSync('subscription_dump.json', txt);
  }
  const data = JSON.parse(txt);
  console.log('Total root objects:', data.length);
  data.slice(0, 5).forEach((item, idx) => {
    console.log(`\n--- Root item ${idx} ---`);
    console.log('remarks:', item.remarks);
    if (item.outbounds) {
      console.log('outbounds count:', item.outbounds.length);
      item.outbounds.forEach((o, i) => {
        console.log(`  outbound ${i}: tag=${o.tag}, protocol=${o.protocol}, server=${o.settings?.vnext?.[0]?.address || o.settings?.servers?.[0]?.address}`);
      });
    }
    if (item.routing) {
      console.log('has routing: rules count =', item.routing.rules?.length);
      console.log('balancers:', item.routing.balancers);
    }
  });
}

main().catch(console.error);

