import type { Answer, Sources, Result } from './app.contract.d.ts';
import { MessagesReplica, nativeCore, grants as replicaGrants, type Records } from './snapback-client';
export const appId = 'com.exact.messages';
export const grants = replicaGrants;
type Message = Result<'conversation'>['messages'][number];
type StoredMessage = Pick<Message,'id'|'body'|'outgoing'|'time'|'day'|'delivery'|'sender'|'reply'|'replyRoot'> & { second:number; order:number; reactions:Record<string,string> };
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
const blocked = new Set<string>();
const localContacts = new Map<string,{first:string,last:string,company:string,phone:string,email:string,notes:string}>();
const deleted = new Set<string>();
const recoverable = new Map<string,{message:StoredMessage,expires:number}[]>();
const recoveryDay = 86400000;
let messageOrder = 0;
const drafts = new Map<string, {draft:string,reply:string}>();
let revision = 0;
let namespace='';
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
  return {second,order:messageOrder++,id,body,outgoing,time,day,delivery:outgoing?'Read':'',sender,reactions:{},reply:'',replyRoot:id};
}
function expireDeleted(now:number) {
  for(const [id,records] of recoverable) {
    const kept=records.filter(r=>r.expires>now);
    if(kept.length)recoverable.set(id,kept);else recoverable.delete(id);
  }
}
function archiveMessages(id:string,rows:StoredMessage[],now:number) {
  expireDeleted(now);
  if(rows.length)recoverable.set(id,[...(recoverable.get(id)||[]),...rows.map(message=>({message,expires:now+30*recoveryDay}))]);
}
function refreshPreview(id:string) {
  const person=people.find(p=>p.id===id),rows=threads.get(id),last=rows?.[rows.length-1];
  if(person){person.preview=last?.body || '';person.time=last?(last.day==='Today'?last.time:last.day):'';}
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
  if(existing) return existing.address?existing:undefined;
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
      const {second:_second,order:_order,reactions:saved,...content}=m;
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
    muted:muted.has(person.id),blocked:blocked.has(person.id),knownContact:!person.id.startsWith('address:') || localContacts.has(person.id),
    contactKind:person.address?.includes('@')?'email':'phone',
    contactAddress:person.address && /^\+1\d{10}$/.test(person.address)?`+1 (${person.address.slice(2,5)}) ${person.address.slice(5,8)}-${person.address.slice(8)}`:person.address || '',
    messages,replies:decorate(rows.filter(m=>m.replyRoot===replying)),revision,scrollRevision:scrollRevisions.get(person.id)||0};
}
const sources: Sources = {
  syncMessages: () => changed(),
  syncState: () => replica?.status() || 'Conversation preview',
  inbox: ([query,_revision])=>({people:people.filter(p=>threads.has(p.id) && !deleted.has(p.id) && p.name.toLowerCase().includes(query.toLowerCase())).map(({address:_address,...p})=>({...p,muted:muted.has(p.id),...(drafts.get(p.id)||{draft:'',reply:''})}))}),
  recentlyDeleted: ([selection,_revision,now])=>{
    expireDeleted(now);
    const ids=new Set(selection.split('|'));
    const rows=people.flatMap(p=>{
      const records=recoverable.get(p.id);if(!records?.length)return [];
      return [{id:p.id,name:p.name,initials:p.initials,color:p.color,count:records.length,
        days:Math.ceil((Math.min(...records.map(r=>r.expires))-now)/recoveryDay),chosen:ids.has(p.id),
        selection:(ids.has(p.id)?[...ids].filter(id=>id!==p.id):[...ids,p.id]).filter(Boolean).join('|')}];
    });
    const chosen=selection?rows.filter(p=>p.chosen):rows;
    return {people:rows,targets:chosen.map(p=>p.id).join('|'),count:chosen.reduce((n,p)=>n+p.count,0)};
  },
  recipients: ([ids,query,body,_revision])=>{
    const selected=selectedPeople(ids),selectedIds=new Set(selected.map(p=>p.id)),text=query.trim();
    const pending=text?(people.find(p=>p.address && p.name.toLowerCase()===text.toLowerCase()) || addressPerson(text)):undefined;
    const resolved=pending?[...selectedIds,...(selectedIds.has(pending.id)?[]:[pending.id])].join('|'):'';
    const matches=people.filter(p=>p.address && !selectedIds.has(p.id) && p.name.toLowerCase().includes(text.toLowerCase()));
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
  blockConversation: ([id,value])=>{
    if(threads.has(id) && !groups.has(id)) {
      if(value){blocked.add(id);pending.delete(id);}else blocked.delete(id);
      revision++;
    }
    return changed();
  },
  createLocalContact: ([first,last,company,phone,email,notes])=>{
    const name=[first.trim(),last.trim()].filter(Boolean).join(' ') || company.trim();
    const initials=[first.trim(),last.trim()].filter(Boolean).map(v=>[...v][0]).join('').toUpperCase() || [...company.trim()].slice(0,2).join('').toUpperCase();
    const addresses=[phone,email].map(addressPerson).filter((p):p is typeof people[number]=>!!p);
    if(!addresses.length && name) addresses.push({id:`contact:${namespace}${++revision}`,name,initials,color:'#92a8ce',preview:'',time:'Now',unread:false});
    for(const candidate of addresses) {
      let person=people.find(p=>p.id===candidate.id);
      if(!person){person=candidate;people.push(person);}
      if(name)person.name=name;
      person.initials=initials;
      localContacts.set(person.id,{first,last,company,phone,email,notes});
    }
    revision++;
    return changed();
  },
  deleteConversation: ([id,now])=>{
    const rows=threads.get(id);
    if(rows){archiveMessages(id,rows,now);threads.set(id,[]);deleted.add(id);pending.delete(id);drafts.delete(id);revision++;}
    return changed();
  },
  recoverConversations: ([selection,now])=>{
    expireDeleted(now);
    for(const id of new Set(selection.split('|'))) {
      const records=recoverable.get(id);if(!records?.length)continue;
      const rows=threads.get(id)||[],existing=new Set(rows.map(m=>m.id));
      threads.set(id,[...rows,...records.map(r=>r.message).filter(m=>!existing.has(m.id))].sort((a,b)=>a.order-b.order));
      recoverable.delete(id);deleted.delete(id);refreshPreview(id);revision++;
    }
    return changed();
  },
  purgeConversations: ([selection,now])=>{
    expireDeleted(now);
    for(const id of new Set(selection.split('|')))if(recoverable.delete(id))revision++;
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
      deleted.delete(id);
    }
    const rows=threads.get(id),person=people.find(p=>p.id===id);
    if(rows && person && body.trim()) {
      const at=fixtureTime(nowMs);
      const item=message(`sent-${namespace}${++revision}`,body,true,at.time,'me','Today',at.second);
      item.delivery='Delivered';
      replyTo(item,rows,reply);
      rows.push(item);person.preview=body;person.time=item.time;person.unread=false;
      drafts.delete(id);
      scrollRevisions.set(id,++scrollGeneration);
      if(!blocked.has(id)) pending.set(id,{start:now+3,end:now+15,reply:reply?item.replyRoot:''});
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
      const item=message(`received-${namespace}${++revision}`,body,false,at.time,sender?.id || '','Today',at.second);
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
  deleteMessages: ([id,selection,now])=>{
    const selected=new Set(selection.split('|')),rows=threads.get(id);
    if(rows){
      const remaining=rows.filter(m=>!selected.has(m.id));
      if(remaining.length!==rows.length){
        archiveMessages(id,rows.filter(m=>selected.has(m.id)),now);
        threads.set(id,remaining);revision++;
        refreshPreview(id);
      }
    }
    return changed();
  },
};
// Persistence contains authored data, never bubble geometry or selection state.
function snapshot():Records {
  const records:Records=new Map();
  people.forEach((person,position)=>records.set(`person:${person.id}`,{kind:'person',person,position,
    conversation:threads.has(person.id),muted:muted.has(person.id),blocked:blocked.has(person.id),deleted:deleted.has(person.id),
    draft:drafts.get(person.id)||null,group:groups.get(person.id)||null,contact:localContacts.get(person.id)||null}));
  const put=(conversation:string,message:StoredMessage,expires:number|null)=>records.set(`message:${encodeURIComponent(conversation)}:${encodeURIComponent(message.id)}`,{kind:'message',conversation,message,expires});
  for(const [id,rows] of threads)for(const message of rows)put(id,message,null);
  for(const [id,rows] of recoverable)for(const row of rows)put(id,row.message,row.expires);
  // Detach the persisted image: source actions mutate the live maps in place.
  return new Map([...records].map(([key,value])=>[key,JSON.parse(JSON.stringify(value))]));
}
function restore(records:Records):void {
  type PersonRecord={kind:'person';person:typeof people[number];position:number;conversation:boolean;muted:boolean;blocked:boolean;deleted:boolean;draft:{draft:string;reply:string}|null;group:string[]|null;contact:typeof localContacts extends Map<string,infer C>?C:null};
  type MessageRecord={kind:'message';conversation:string;message:StoredMessage;expires:number|null};
  const persons:PersonRecord[]=[],messages:MessageRecord[]=[];
  for(const value of records.values()){
    if(!value || typeof value!=='object')throw new Error('Invalid Messages replica record');
    const row=JSON.parse(JSON.stringify(value)) as PersonRecord|MessageRecord;
    if(row.kind==='person'){
      if(!row.person || !['id','name','initials','color','preview','time'].every(k=>typeof (row.person as unknown as Record<string,unknown>)[k]==='string') || typeof row.person.unread!=='boolean' || !Number.isFinite(row.position))throw new Error('Invalid Messages contact record');
      persons.push(row);
    }else if(row.kind==='message'){
      if(!row.message || !['id','body','time','day','delivery','sender','reply','replyRoot'].every(k=>typeof (row.message as unknown as Record<string,unknown>)[k]==='string') || typeof row.message.outgoing!=='boolean' || !Number.isFinite(row.message.order) || !Number.isFinite(row.message.second) || !row.message.reactions || Object.values(row.message.reactions).some(v=>typeof v!=='string') || (row.expires!==null && !Number.isFinite(row.expires)))throw new Error('Invalid Messages message record');
      messages.push(row);
    }else throw new Error('Unknown Messages replica record');
  }
  people.splice(0);threads.clear();muted.clear();blocked.clear();deleted.clear();drafts.clear();groups.clear();localContacts.clear();recoverable.clear();
  for(const row of persons.sort((a,b)=>a.position-b.position || a.person.id.localeCompare(b.person.id))){
    const id=row.person.id;people.push(row.person);
    if(row.conversation)threads.set(id,[]);
    if(row.muted)muted.add(id);if(row.blocked)blocked.add(id);if(row.deleted)deleted.add(id);
    if(row.draft)drafts.set(id,row.draft);if(row.group)groups.set(id,row.group);if(row.contact)localContacts.set(id,row.contact);
  }
  for(const row of messages.sort((a,b)=>a.message.order-b.message.order || a.message.id.localeCompare(b.message.id))){
    if(row.expires!==null){const rows=recoverable.get(row.conversation)||[];rows.push({message:row.message,expires:row.expires});recoverable.set(row.conversation,rows);}
    else {const rows=threads.get(row.conversation)||[];rows.push(row.message);threads.set(row.conversation,rows);}
    messageOrder=Math.max(messageOrder,row.message.order+1);
  }
  for(const id of pending.keys())if(deleted.has(id)||blocked.has(id)||!threads.has(id))pending.delete(id);
  revision++;
}
let replica:MessagesReplica|undefined;
let configuredCore:ReturnType<typeof nativeCore>;
let opened=false;
let tail:Promise<unknown>=Promise.resolve();
function local<T>(work:()=>Promise<T>):Promise<T>{const result=tail.then(work);tail=result.catch(()=>{});return result;}
export const answer: Answer = (source,args,store,storage,native) => {
  // The bake and unconfigured unit-test host deliberately expose no storage;
  // the native hook records that external dependency before refusing it.
  if(!opened){configuredCore=nativeCore(native);opened=configuredCore!==null;}
  const core=configuredCore;
  if(core===null)return sources[source](args,store,storage,native);
  const ready=async()=>{
    if(configuredCore===null)return undefined;
    if(!replica){
      try{const client=await MessagesReplica.open(storage,core);namespace=client.namespace;await client.seed(snapshot());restore(client.initial());replica=client;}
      catch(error){
        if((error instanceof Error?error.message:String(error))!=='storage is unavailable in agent mode')throw error;
        configuredCore=null;opened=true;return undefined;
      }
    }
    return replica;
  };
  // Network awaits never hold the local action queue. Only applying a received
  // page and replacing the model enter the same short gate as local edits.
  if(source==='syncMessages')return local(ready).then(async client=>{if(!client)return changed();const previous=client.status();await client.sync(Number(args[0]),local,restore);if(previous!==client.status())revision++;return changed();});
  return local(async()=>{
    const client=await ready();
    if(!client)return sources[source](args,store,storage,native);
    const previousPending=new Map(pending),previousTicks=ticks;
    const value=await sources[source](args,store,storage,native);
    try{await client.persist(snapshot());}catch(error){
      restore(client.initial());pending.clear();for(const [id,activity] of previousPending)if(!deleted.has(id)&&!blocked.has(id)&&threads.has(id))pending.set(id,activity);ticks=previousTicks;
      // A failed save must not leave an invalid model that every later read
      // tries to save again. Report the original error even if disk is full.
      try{await client.failed(error);}catch{/* The runner still receives the save failure. */}
      throw error;
    }
    return value;
  });
};
