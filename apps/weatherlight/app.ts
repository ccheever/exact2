import type { Answer, Sources, Result } from './app.contract.d.ts';

export const appId = 'com.exact.weatherlight';
export const grants = 'net.fetch https://api.open-meteo.com\nnet.fetch https://geocoding-api.open-meteo.com';
type Forecast = Result<'forecast'>;
type Hour = Forecast['current'];
type Outlook = Result<'outlook'>;
type Json = Record<string, unknown>;
const cache = new Map<string, Forecast>();
const blankHour: Hour = {id:'now',time:'',temp:0,feels:0,humidity:0,cloud:30,rain:0,chance:0,wind:0,code:0,day:1,hour:12};
const empty: Forecast = {lat:0,lon:0,ready:false,stale:false,message:'',updated:'',current:blankHour,hours:[],days:[]};
function object(v: unknown): Json {
  if (!v || typeof v !== 'object' || Array.isArray(v)) throw new Error('Incomplete forecast');
  return v as Json;
}
function numeric(v: unknown): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) throw new Error('Incomplete forecast');
  return v;
}
function string(v: unknown): string {
  if (typeof v !== 'string') throw new Error('Incomplete forecast');
  return v;
}
function at(row: Json, key: string, i: number): unknown {
  if (!Array.isArray(row[key])) throw new Error('Incomplete forecast');
  return (row[key] as unknown[])[i];
}
function times(row: Json): string[] {
  if (!Array.isArray(row.time) || row.time.length === 0) throw new Error('Incomplete forecast');
  return row.time.map(string);
}
const clamp = (n: number, min: number, max: number) => Math.max(min,Math.min(max,n));
function hourOf(time: string): number {
  if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d$/.test(time)) throw new Error('Invalid forecast time');
  return Number(time.slice(11,13)) + Number(time.slice(14,16))/60;
}
function clock(time: string): string {
  if (!time) return '—';
  const hour = Math.floor(hourOf(time));
  return `${hour%12 || 12}:${time.slice(14,16)} ${hour < 12 ? 'am' : 'pm'}`;
}
function hourLabel(time: string): string {
  const hour = Math.floor(hourOf(time));
  return `${hour%12 || 12}${hour < 12 ? 'am' : 'pm'}`;
}
function condition(code: number, day = 1): string {
  if (code === 0) return day ? 'Clear skies' : 'A clear night';
  if (code === 1) return 'Mostly clear';
  if (code === 2) return 'Partly cloudy';
  if (code === 3) return 'Overcast';
  if (code === 45 || code === 48) return 'Foggy';
  if (code >= 51 && code <= 57) return 'Drizzle';
  if (code >= 61 && code <= 67) return 'Rain';
  if (code >= 71 && code <= 77) return 'Snow';
  if (code >= 80 && code <= 82) return 'Rain showers';
  if (code >= 85 && code <= 86) return 'Snow showers';
  if (code >= 95) return 'Thunderstorms';
  return 'Variable skies';
}
function icon(code: number, day = 1): string {
  if (code <= 1) return day ? '☀' : '☾';
  if (code === 2) return '◒';
  if (code <= 48) return '☁';
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return '❄';
  if (code >= 95) return 'ϟ';
  return '☂';
}
async function json(url: string): Promise<Json> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Weather service returned ${response.status}`);
  return object(await response.json());
}
// Open-Meteo returns ISO wall times in the requested city's timezone. Keep
// those strings intact: converting through the device timezone shifts the day.
async function forecast(lat: number, lon: number): Promise<Forecast> {
  const key = `${lat},${lon}`;
  try {
    if (!Number.isFinite(lat) || !Number.isFinite(lon) || Math.abs(lat)>90 || Math.abs(lon)>180) throw new Error('Invalid location');
    const common = 'temperature_2m,relative_humidity_2m,apparent_temperature,is_day,precipitation,weather_code,cloud_cover,wind_speed_10m';
    const url = `https://api.open-meteo.com/v1/forecast?latitude=${lat}&longitude=${lon}&timezone=auto&forecast_days=7&current=${common}&hourly=${common},precipitation_probability&daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,precipitation_probability_max`;
    const data = await json(url), c = object(data.current), h = object(data.hourly), d = object(data.daily);
    const currentTime = string(c.time);
    const all = times(h).map((time,i): Hour => ({id:time,time,temp:numeric(at(h,'temperature_2m',i)),feels:numeric(at(h,'apparent_temperature',i)),humidity:numeric(at(h,'relative_humidity_2m',i)),cloud:numeric(at(h,'cloud_cover',i)),rain:numeric(at(h,'precipitation',i)),chance:numeric(at(h,'precipitation_probability',i)),wind:numeric(at(h,'wind_speed_10m',i)),code:numeric(at(h,'weather_code',i)),day:numeric(at(h,'is_day',i)),hour:hourOf(time)}));
    const thisHour = all.find(hour=>hour.time.slice(0,13)===currentTime.slice(0,13));
    if (!thisHour) throw new Error('Current hour missing');
    const current: Hour = {id:'now',time:currentTime,temp:numeric(c.temperature_2m),feels:numeric(c.apparent_temperature),humidity:numeric(c.relative_humidity_2m),cloud:numeric(c.cloud_cover),rain:numeric(c.precipitation),chance:thisHour.chance,wind:numeric(c.wind_speed_10m),code:numeric(c.weather_code),day:numeric(c.is_day),hour:hourOf(currentTime)};
    const days = times(d).slice(0,7).map((id,i)=>({id,low:numeric(at(d,'temperature_2m_min',i)),high:numeric(at(d,'temperature_2m_max',i)),code:numeric(at(d,'weather_code',i)),chance:numeric(at(d,'precipitation_probability_max',i)),sunrise:string(at(d,'sunrise',i)),sunset:string(at(d,'sunset',i)),uv:numeric(at(d,'uv_index_max',i))}));
    if(days.length!==7) throw new Error('Incomplete weekly forecast');
    const result: Forecast = {lat,lon,ready:true,stale:false,message:'',updated:currentTime.replace('T',' · '),current,hours:all.filter(hour=>hour.time>currentTime).slice(0,24),days};
    if(result.hours.length!==24) throw new Error('Incomplete hourly forecast');
    // Bounded, location-specific cache: a failed refresh may keep that city's
    // last good reading; it never relabels another city's forecast.
    cache.delete(key); cache.set(key,result);
    if(cache.size>8) cache.delete(cache.keys().next().value!);
    return result;
  } catch {
    const previous = cache.get(key);
    return {...(previous || empty),lat,lon,stale:!!previous,message:previous ? 'Could not refresh. Showing this city’s last available forecast.' : 'The forecast couldn’t be reached. Check your connection and try again.'};
  }
}
function outlook(f: Forecast, units: string, selected: string, lat: number, lon: number): Outlook {
  const temp = (n:number) => `${Math.round(units==='F' ? n*9/5+32 : n)}°`;
  const blank: Outlook = {ready:false,temp:'—°',condition:'',feels:'—',wind:'—',humidity:'—',chance:'—',range:'',sunrise:'—',sunset:'—',uv:'—',summary:'',selectedLabel:'RIGHT NOW',cloud:30,rain:0,day:1,hour:12,speed:5,
    hours:Array.from({length:24},(_,i)=>({id:`waiting-${i}`,label:'—',temp:'—°',icon:'—',chance:'—'})),
    days:Array.from({length:7},(_,i)=>({id:`waiting-${i}`,label:i===0?'Today':'—',condition:'Forecast unavailable',icon:'—',chance:'—',low:'—°',high:'—°',left:'0%',span:'0%'}))};
  if(!f.ready || f.lat!==lat || f.lon!==lon || !f.days.length) return blank;
  const h = f.hours.find(hour=>hour.id===selected) || f.current;
  const day = f.days.find(d=>d.id===h.time.slice(0,10)) || f.days[0];
  const min = Math.min(...f.days.map(d=>d.low)), max = Math.max(...f.days.map(d=>d.high)), spread = Math.max(1,max-min);
  const wind = `${Math.round(units==='F' ? h.wind/1.609344 : h.wind)} ${units==='F' ? 'mph' : 'km/h'}`;
  const summary = h.code>=95 ? 'Thunderstorms in the forecast. Keep an eye on local alerts.' : h.rain>0 ? 'A little weather moving through. Bring a layer and an umbrella.' : h.chance>=50 ? 'Rain may be on the way. An umbrella could come in handy.' : h.wind>30 ? 'A breezy stretch ahead. Take a layer for the wind.' : !h.day ? 'The day gives way to a quieter sky.' : h.cloud>70 ? 'A soft blanket of cloud over the city.' : 'Some room for sunshine. A good moment to step outside.';
  return {ready:true,temp:temp(h.temp),condition:condition(h.code,h.day),feels:temp(h.feels),wind,humidity:`${Math.round(h.humidity)}%`,chance:`${Math.round(h.chance)}%`,range:`H ${temp(day.high)}   L ${temp(day.low)}  ·  Feels like ${temp(h.feels)}`,sunrise:clock(day.sunrise),sunset:clock(day.sunset),uv:`${Math.round(day.uv)} · ${day.uv<3 ? 'Low' : day.uv<6 ? 'Moderate' : day.uv<8 ? 'High' : 'Very high'}`,summary,selectedLabel:h.id==='now' ? 'RIGHT NOW' : `${h.time.slice(5,10)} · ${clock(h.time)} FORECAST`,cloud:clamp(h.cloud,0,100),rain:clamp(h.rain,0,20),day:h.day,hour:h.hour,speed:clamp(h.wind,0,150),hours:f.hours.map(hour=>({id:hour.id,label:hourLabel(hour.time),temp:temp(hour.temp),icon:icon(hour.code,hour.day),chance:`${Math.round(hour.chance)}%`})),days:f.days.map((d,i)=>({id:d.id,label:i===0?'Today':['Sun','Mon','Tue','Wed','Thu','Fri','Sat'][new Date(d.id+'T12:00:00Z').getUTCDay()],condition:condition(d.code),icon:icon(d.code),chance:`${Math.round(d.chance)}%`,low:temp(d.low),high:temp(d.high),left:`${(d.low-min)/spread*100}%`,span:`${Math.max(3,(d.high-d.low)/spread*100)}%`}))};
}
async function places(query: string): Promise<Result<'places'>> {
  const term = query.trim();
  if(term.length<2) return {items:[],message:'Search for a city, or choose one of the places below.'};
  try {
    const data = await json(`https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(term.slice(0,100))}&count=6&language=en&format=json`);
    const rows = data.results === undefined ? [] : data.results;
    if(!Array.isArray(rows)) throw new Error('Invalid search results');
    const items = rows.map(value=>{
      const p=object(value),lat=numeric(p.latitude),lon=numeric(p.longitude);
      if(Math.abs(lat)>90 || Math.abs(lon)>180) throw new Error('Invalid location');
      return {id:String(numeric(p.id)),name:string(p.name),detail:[p.admin1,p.country].filter(v=>typeof v==='string' && v!=='').join(', '),lat,lon};
    });
    return {items,message:items.length ? 'Choose a place to see its forecast.' : 'No places found. Try another city or postal code.'};
  } catch {return {items:[],message:'City search couldn’t connect. Try again, or choose a city below.'};}
}
const sources: Sources = {
  forecast: ([lat,lon,revision]) => revision === 0 ? {...empty,lat,lon} : forecast(lat,lon),
  outlook: ([f,units,selected,lat,lon]) => outlook(f,units,selected,lat,lon),
  places: ([query]) => places(query),
};
export const answer: Answer = (source,args,store,storage) => sources[source](args,store,storage);
