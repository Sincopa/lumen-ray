export function formatMinutesAgoRu(minutes: number): string {
  if (minutes < 60) {
    const mod10 = minutes % 10;
    const mod100 = minutes % 100;
    let unit = 'минут';
    if (mod10 === 1 && mod100 !== 11) {
      unit = 'минуту';
    } else if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
      unit = 'минуты';
    }
    return `Обновлено: ${minutes} ${unit} назад`;
  }

  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    const mod10 = hours % 10;
    const mod100 = hours % 100;
    let unit = 'часов';
    if (mod10 === 1 && mod100 !== 11) {
      unit = 'час';
    } else if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
      unit = 'часа';
    }
    return `Обновлено: ${hours} ${unit} назад`;
  }

  const days = Math.floor(hours / 24);
  const mod10 = days % 10;
  const mod100 = days % 100;
  let unit = 'дней';
  if (mod10 === 1 && mod100 !== 11) {
    unit = 'день';
  } else if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) {
    unit = 'дня';
  }
  return `Обновлено: ${days} ${unit} назад`;
}

export function formatMinutesAgoEn(minutes: number): string {
  if (minutes < 60) {
    return `Updated: ${minutes} ${minutes === 1 ? 'minute' : 'minutes'} ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `Updated: ${hours} ${hours === 1 ? 'hour' : 'hours'} ago`;
  }
  const days = Math.floor(hours / 24);
  return `Updated: ${days} ${days === 1 ? 'day' : 'days'} ago`;
}

export function formatSubscriptionUpdatedAgo(
  updatedAt?: number,
  updatedMinutesAgo?: number,
  lang: string = 'ru',
): string {
  let minutes = updatedMinutesAgo;
  if (minutes === undefined) {
    if (!updatedAt || updatedAt === 0) {
      minutes = 1;
    } else {
      const elapsed = Date.now() - updatedAt;
      minutes = Math.max(1, Math.floor(elapsed / 60000));
    }
  }

  return lang === 'ru'
    ? formatMinutesAgoRu(minutes)
    : formatMinutesAgoEn(minutes);
}
