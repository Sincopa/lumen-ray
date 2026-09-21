import fs from 'fs';

const data = JSON.parse(fs.readFileSync('subscription_dump.json', 'utf8'));

function cleanName(name) {
  return name.replace(/^[\u{1F1E6}-\u{1F1FF}]{2}\s*/u, '')
             .replace(/^[🏳️🇪🇺\s/]+/u, '')
             .replace(/^[\u{1F1E6}-\u{1F1FF}]{2}\s*/u, '')
             .trim();
}

data.forEach((item, idx) => {
  console.log(`[${idx}] '${item.remarks}' -> '${cleanName(item.remarks)}'`);
});

