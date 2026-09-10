import type { Answer, Sources, Result } from './app.contract.d.ts';
export const appId = 'com.exact.messages';
export const grants = '';
type Message = Result<'conversation'>['messages'][number];
type StoredMessage = Omit<Message,'reaction'|'reactionCount'|'reactionEntries'|'reactionGroups'|'reactionPanelWidth'> & { second:number; reactions:Record<string,string> };
type Person = Result<'inbox'>['people'][number];
const people: (Omit<Person,'draft'|'reply'|'muted'> & {address?:string})[] = [
  {id:'maya',address:'+14155550101',name:'Maya Chen',initials:'MC',color:'#a58ac4',preview:'See you there! ☕️',time:'9:41 AM',unread:true},
  {id:'weekend',name:'Weekend people',initials:'☀',color:'#e2a34d',preview:'Jules: I’ll bring sandwiches 🥪',time:'9:33 AM',unread:true},
  {id:'dad',address:'+14155550102',name:'Dad',initials:'D',color:'#829baa',preview:'Loved seeing you this weekend ❤️',time:'Yesterday',unread:false},
  {id:'alex',address:'+14155550103',name:'Alex Rivera',initials:'AR',color:'#76a998',preview:'You sent a photo',time:'Yesterday',unread:false},
  {id:'jules',address:'+14155550104',name:'Jules',initials:'J',color:'#cd8f99',preview:'That sounds perfect',time:'Monday',unread:false},
  {id:'sam',address:'+14155550105',name:'Sam',initials:'S',color:'#8196c0',preview:'Thanks again!',time:'Monday',unread:false},
];
const reactions = [
  {id:'heart',value:'❤️',label:'Love'}, {id:'like',value:'👍',label:'Like'},
  {id:'dislike',value:'👎',label:'Dislike'}, {id:'laugh',value:'haha',label:'Laugh'},
  {id:'emphasis',value:'‼️',label:'Emphasize'}, {id:'question',value:'❓',label:'Question'},
  {id:'laugh-face',value:'😂',label:'Laughing face'},
  {id:'smile',value:'😊',label:'Smile'}, {id:'fire',value:'🔥',label:'Fire'},
  {id:'party',value:'🎉',label:'Celebrate'}, {id:'eyes',value:'👀',label:'Eyes'},
  {id:'hundred',value:'💯',label:'One hundred'}, {id:'thanks',value:'🙏',label:'Thank you'},
];
const threads = new Map<string, StoredMessage[]>();
const muted = new Set<string>();
const deleted = new Set<string>();
const drafts = new Map<string, {draft:string,reply:string}>();
let revision = 0;
const pending = new Map<string, {start:number,end:number,reply:string}>();
let ticks = 0;
const changed = () => ({revision,pending:pending.size>0});
const scrollRevisions = new Map(people.map((person,index)=>[person.id,index]));
let scrollGeneration = people.length;
// Fixture dates stay fixed; precise within-day times decide bubble runs.
const fixtureStart = 9*3600+42*60;
function clockSecond(time:string) {
  const [,hour,minute,period]=/^(\d+):(\d+) (AM|PM)$/.exec(time)!;
  return (Number(hour)%12+(period==='PM'?12:0))*3600+Number(minute)*60;
}
function fixtureTime(nowMs:number) {
  const second=fixtureStart+nowMs/1000,minute=Math.floor(second/60),hour=Math.floor(minute/60)%24;
  return {second,time:`${hour%12 || 12}:${String(minute%60).padStart(2,'0')} ${hour<12?'AM':'PM'}`};
}
function sameRun(a:StoredMessage|undefined,b:StoredMessage|undefined) {
  return !!a && !!b && a.sender===b.sender && a.outgoing===b.outgoing && a.day===b.day && b.second>=a.second && b.second-a.second<60;
}
function message(id:string,body:string,outgoing:boolean,time:string,sender=outgoing?'me':'',day='Today',second=clockSecond(time)): StoredMessage {
  return {second,chosen:false,selection:"",id,body,outgoing,time,day,timeLabel:'',delivery:outgoing?'Read':'',sender,senderName:'',senderInitials:'',senderColor:'',showSender:false,reactions:{},reply:'',tail:true,replyRoot:id,replyCount:0};
}
function replyTo(item:StoredMessage,rows:StoredMessage[],root:string) {
  if(!root) return;
  const parent=rows.find(m=>m.id===root);
  // A thread keeps its identity when its original bubble is deleted.
  item.replyRoot=parent?.replyRoot || root;
  item.reply=parent?.body || rows.find(m=>m.replyRoot===root)?.reply || '';
}
threads.set('maya',[
  message('m1','Hey! Are you around this morning?',false,'9:30 AM'),
  message('m2','Yeah! Just finishing a few things',true,'9:31 AM'),
  message('m3','Want to grab coffee? There’s a new place on Valencia I’ve been wanting to try',false,'9:32 AM'),
  message('m4','The one with the little green door?',true,'9:33 AM'),
  message('m5','Yes!! That’s the one',false,'9:34 AM'),
  message('m6','I walked past it yesterday. It looks so good',false,'9:34 AM'),
  message('m7','I’m in. Meet you there at 10?',true,'9:36 AM'),
  message('m8','Perfect 😊',false,'9:37 AM'),
  message('m9','I’ll get us a table',true,'9:40 AM'),
  message('m10','See you there! ☕️',false,'9:41 AM'),
]);
threads.get('maya')![6].reactions.maya='❤️';
for(const person of people.slice(2)) threads.set(person.id,[
  message(`${person.id}-1`,'Hey! How’s your week going?',false,'9:20 AM','',person.time),
  message(`${person.id}-2`,'Really good! How about yours?',true,'9:24 AM','me',person.time),
  message(`${person.id}-3`,person.preview,false,'9:32 AM','',person.time),
]);
threads.set('weekend',[
  message('weekend-1','Anyone up for a hike on Saturday?',false,'9:20 AM','maya'),
  message('weekend-2','Definitely! Count me in 🌲',true,'9:24 AM'),
  message('weekend-3','Who’s bringing snacks?',false,'9:32 AM','alex'),
  message('weekend-4','I can bring fruit.',false,'9:32 AM','alex'),
  message('weekend-5','I’ll bring sandwiches 🥪',false,'9:33 AM','jules'),
]);
// Member sets give a new compose session the same local thread in any order.
const groups = new Map<string,string[]>([['weekend',['alex','jules','maya']]]);
// Local address identity only: the example never looks up service availability.
function addressPerson(value:string):typeof people[number] | undefined {
  const text=value.trim();
  let address='',name='';
  if(/^\+?[0-9() .-]+$/.test(text)) {
    const digits=text.replace(/\D/g,'');
    if(digits.length<3 || digits.length>15) return;
    address=text.startsWith('+')?`+${digits}`:digits.length===10?`+1${digits}`:digits.length===11 && digits[0]==='1'?`+${digits}`:digits;
    name=/^\+1\d{10}$/.test(address)?`+1 (${address.slice(2,5)}) ${address.slice(5,8)}-${address.slice(8)}`:address;
  } else if(/^[^\s@,;|<>]+@[^\s@,;|<>]+\.[^\s@,;|<>]+$/.test(text)) {
    address=text.toLowerCase();name=address;
  } else return;
  return people.find(p=>p.address===address) || {id:`address:${encodeURIComponent(address)}`,address,name,initials:'',color:'#829baa',preview:'',time:'Now',unread:false};
}
function recipientById(id:string) {
  const existing=people.find(p=>p.id===id);
  if(existing) return existing;
  if(!id.startsWith('address:')) return;
  try {
    const person=addressPerson(decodeURIComponent(id.slice(8)));
    if(person?.id===id) return person;
  } catch { /* An invalid encoded address is not a recipient. */ }
}
function selectedPeople(ids:string) {
  return [...new Set(ids.split('|'))].filter(id=>!groups.has(id)).map(recipientById).filter((p):p is typeof people[number]=>!!p);
}
function recipientTarget(selected:typeof people) {
  const ids=selected.map(p=>p.id).sort();
  if(ids.length<2) return ids[0] || '';
  return [...groups].find(([,members])=>members.join('|')===ids.join('|'))?.[0] || `group:${ids.join('|')}`;
}
function ensureConversation(id:string) {
  if(threads.has(id)) return;
  const address=recipientById(id);
  if(address) {
    if(!people.some(p=>p.id===id)) people.unshift(address);
    threads.set(id,[]);
    return;
  }
  if(!id.startsWith('group:')) return;
  const members=selectedPeople(id.slice(6));
  if(members.length<2 || recipientTarget(members)!==id) return;
  groups.set(id,members.map(p=>p.id).sort());
  people.unshift({id,name:members.map(p=>p.name.split(' ')[0]).join(', '),initials:members.slice(0,2).map(p=>p.initials[0]).join(''),color:'#829baa',preview:'',time:'Now',unread:false});
  threads.set(id,[]);
}
function responder(id:string) {
  return recipientById(groups.get(id)?.[0] || id);
}
function conversation(id:string,replying:string,selection:string):Result<'conversation'> {
  const person=people.find(p=>p.id===id) || people[0];
  const rows=threads.get(person.id) || [];
  const selected=new Set(selection.split("|").filter(id=>rows.some(m=>m.id===id)));
  const decorate=(visible:StoredMessage[])=>{
    let lastOutgoing=-1;
    visible.forEach((m,i)=>{if(m.outgoing)lastOutgoing=i;});
    return visible.map((m,i)=>{
      const sender=groups.has(person.id) && !m.outgoing?recipientById(m.sender):undefined;
      const startsDay=!visible[i-1] || visible[i-1].day!==m.day;
      const {second:_second,reactions:saved,...content}=m;
      const entries=Object.entries(saved).sort(([a],[b])=>a==='me'?-1:b==='me'?1:0).map(([id,value],i)=>{
        const who=id==='me'?{name:'You',initials:'ME',color:'#859bc1'}:recipientById(id);
        return {id,value,name:who?.name||id,initials:who?.initials||'',color:who?.color||'#829baa',own:id==='me',offset:i*27,edge:i===0};
      });
      const byValue=new Map<string,typeof entries>();
      for(const entry of entries){const group=byValue.get(entry.value)||[];group.push({...entry,offset:group.length*18});byValue.set(entry.value,group);}
      const reactionGroups=[...byValue].map(([value,people])=>({value,people,width:32+(people.length-1)*18}));
      return {...content,reaction:saved.me||'',reactionCount:entries.length,reactionEntries:entries,reactionGroups,reactionPanelWidth:Math.max(124,18+reactionGroups.length*98),chosen:selected.has(m.id),selection:(selected.has(m.id)?[...selected].filter(id=>id!==m.id):[...selected,m.id]).join("|"),timeLabel:startsDay?`${m.day} ${m.time}`:'',delivery:i===lastOutgoing?m.delivery:'',
        tail:!sameRun(m,visible[i+1]),
        senderName:sender?.name || '',senderInitials:sender?.initials || '',senderColor:sender?.color || '',
        showSender:!!sender && (startsDay || visible[i-1].sender!==m.sender),
        replyCount:rows.filter(r=>r.id!==m.id && r.replyRoot===m.id).length};
    });
  };
  const messages=decorate(rows);
  const activity=pending.get(person.id);
  const typing=!!activity && ticks>=activity.start;
  return {selectedText:rows.filter(m=>selected.has(m.id)).map(m=>m.body).join("\n"),selectionCount:selected.size,typingName:typing?(responder(person.id)?.name || ''):'',
    typingAvatar:typing && groups.has(person.id)?responder(person.id)!.initials:'',typingRoot:typing?activity!.reply:'',
    id:person.id,name:person.name,initials:person.initials,color:person.color,reactions,
    muted:muted.has(person.id),knownContact:!person.id.startsWith('address:'),
    contactKind:person.address?.includes('@')?'email':'phone',
    contactAddress:person.address && /^\+1\d{10}$/.test(person.address)?`+1 (${person.address.slice(2,5)}) ${person.address.slice(5,8)}-${person.address.slice(8)}`:person.address || '',
    messages,replies:decorate(rows.filter(m=>m.replyRoot===replying)),revision,scrollRevision:scrollRevisions.get(person.id)||0};
}
const sources: Sources = {
  inbox: ([query,_revision])=>({people:people.filter(p=>!deleted.has(p.id) && p.name.toLowerCase().includes(query.toLowerCase())).map(({address:_address,...p})=>({...p,muted:muted.has(p.id),...(drafts.get(p.id)||{draft:'',reply:''})}))}),
  recipients: ([ids,query,body,_revision])=>{
    const selected=selectedPeople(ids),selectedIds=new Set(selected.map(p=>p.id)),text=query.trim();
    const pending=text?(people.find(p=>!groups.has(p.id) && p.name.toLowerCase()===text.toLowerCase()) || addressPerson(text)):undefined;
    const resolved=pending?[...selectedIds,...(selectedIds.has(pending.id)?[]:[pending.id])].join('|'):'';
    const matches=people.filter(p=>!groups.has(p.id) && !selectedIds.has(p.id) && p.name.toLowerCase().includes(text.toLowerCase()));
    if(pending && !selectedIds.has(pending.id) && !matches.some(p=>p.id===pending.id)) matches.unshift(pending);
    return {selected:selected.map(p=>({id:p.id,name:p.name,without:selected.filter(other=>other.id!==p.id).map(p=>p.id).join('|')})),
      people:matches.map(({address:_address,...p})=>({...p,draft:'',reply:'',muted:false})),resolved,
      last:selected[selected.length-1]?.id || '',withoutLast:selected.slice(0,-1).map(p=>p.id).join('|'),
      target:recipientTarget(resolved?selectedPeople(resolved):selected),canSend:(selected.length>0 || !!pending) && (!text || !!pending) && !!body.trim()};
  },
  conversation: ([id,_revision,replying,selection])=>conversation(id,replying,selection),
  markRead: ([id])=>{const p=people.find(p=>p.id===id);if(p)p.unread=false;revision++;return changed();},
  setConversationUnread: ([id,unread])=>{
    const person=people.find(p=>p.id===id && !deleted.has(id));
    if(person && person.unread!==unread){person.unread=unread;revision++;}
    return changed();
  },
  muteConversation: ([id])=>{
    if(threads.has(id)){if(muted.has(id))muted.delete(id);else muted.add(id);revision++;}
    return changed();
  },
  deleteConversation: ([id])=>{
    if(threads.has(id)){deleted.add(id);pending.delete(id);drafts.delete(id);revision++;}
    return changed();
  },
  saveDraft: ([id,draft,reply])=>{
    if(threads.has(id)) {
      if(draft || reply) drafts.set(id,{draft,reply}); else drafts.delete(id);
      revision++;
    }
    return changed();
  },
  sendMessage: ([id,body,reply,now,nowMs])=>{
    if(body.trim()) {
      ensureConversation(id);
      if(deleted.delete(id)) threads.set(id,[]);
    }
    const rows=threads.get(id),person=people.find(p=>p.id===id);
    if(rows && person && body.trim()) {
      const at=fixtureTime(nowMs);
      const item=message(`sent-${++revision}`,body,true,at.time,'me','Today',at.second);
      item.delivery='Delivered';
      replyTo(item,rows,reply);
      rows.push(item);person.preview=body;person.time=item.time;person.unread=false;
      drafts.delete(id);
      scrollRevisions.set(id,++scrollGeneration);
      pending.set(id,{start:now+3,end:now+15,reply:reply?item.replyRoot:''});
    }
    return changed();
  },
  advanceReplies: ([now,activeThread,nowMs])=>{
    const previous=ticks;
    ticks=now;
    for(const [id,activity] of pending) {
      if(previous<activity.start && now>=activity.start) {
        for(const item of threads.get(id) || []) if(item.outgoing) item.delivery='Read';
        revision++;
      }
      if(now<activity.end) continue;
      const rows=threads.get(id)!, person=people.find(p=>p.id===id)!;
      const body=id==='weekend'?'Sounds good! 🌲':id==='maya'?'See you soon! ☕️':'Sounds good 😊';
      const sender=groups.has(id)?responder(id):undefined;
      const at=fixtureTime(nowMs);
      const item=message(`received-${++revision}`,body,false,at.time,sender?.id || '','Today',at.second);
      replyTo(item,rows,activity.reply);
      rows.push(item);person.preview=sender?`${sender.name.split(' ')[0]}: ${body}`:body;person.time=item.time;person.unread=id!==activeThread;
      pending.delete(id);
      // Incoming activity follows only an already-pinned reader; no scroll command.
    }
    return changed();
  },
  react: ([id,messageId,emoji])=>{
    const m=threads.get(id)?.find(m=>m.id===messageId);
    if(m){if(m.reactions.me===emoji)delete m.reactions.me;else m.reactions.me=emoji;revision++;}
    return changed();
  },
  deleteMessages: ([id,selection])=>{
    const selected=new Set(selection.split('|')),rows=threads.get(id);
    if(rows){
      const remaining=rows.filter(m=>!selected.has(m.id));
      if(remaining.length!==rows.length){
        threads.set(id,remaining);revision++;
        const person=people.find(p=>p.id===id),last=remaining[remaining.length-1];
        if(person){person.preview=last?.body || '';person.time=last?(last.day==='Today'?last.time:last.day):'';}
      }
    }
    return changed();
  },
};
export const answer: Answer = (source,args,store,storage) => sources[source](args,store,storage);
