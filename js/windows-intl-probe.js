(function () {
  var assertions = 0;
  var observations = [];
  var failures = [];
  function ok(value, label) { if (!value) failures.push(label); ++assertions; }
  function eq(a,b,label) { ok(a===b,label+': '+JSON.stringify(a)+' != '+JSON.stringify(b)); }
  function refuses(action, label) { var failed=false; try { action(); } catch(e) { failed=true; } ok(failed,label); }
  function joined(formatter, value, label) {
    var parts=formatter.formatToParts(value);
    eq(parts.map(function(p){return p.value}).join(''),formatter.format(value),label+' concat');
    ok(parts.length>0 && parts.every(function(p){return typeof p.type==='string' && p.value.length>0}),label+' parts');
    return parts;
  }
  ['en-US','fr-FR','de-DE'].forEach(function(locale) {
    var dt=new Intl.DateTimeFormat(locale,{timeZone:'UTC',year:'numeric',month:'long',day:'2-digit',weekday:'long',hour:'2-digit',minute:'2-digit',second:'2-digit',fractionalSecondDigits:3,hourCycle:'h23'});
    var parts=joined(dt,Date.UTC(2020,1,3,14,5,6,123),locale+' date');
    ok(parts.some(function(p){return p.type==='year' && p.value==='2020'}),locale+' actual year');
    ok(parts.some(function(p){return p.type==='fractionalSecond' && p.value==='123'}),locale+' milliseconds');
    eq(dt.resolvedOptions().calendar,'gregory',locale+' Gregorian');
    eq(dt.resolvedOptions().hourCycle,'h23',locale+' hour cycle');
    observations.push({locale:locale,date:dt.format(Date.UTC(2020,1,3,14,5,6,123)),options:dt.resolvedOptions()});
    ['decimal','percent'].forEach(function(style){
      var nf=new Intl.NumberFormat(locale,{style:style});
      [-1234.567,0,-0,NaN,Infinity,-Infinity].forEach(function(n){joined(nf,n,locale+' '+style+' '+n)});
    });
  });
  eq(new Intl.DateTimeFormat('en-US',{timeZone:'UTC',year:'numeric'}).formatToParts(0).find(function(p){return p.type==='year'}).value,'1970','date epoch parts');
  eq(new Date(0).toLocaleDateString('en-US',{timeZone:'UTC',year:'numeric'}),'1970','Date.toLocaleDateString');
  eq((1234.5).toLocaleString('de-DE'),'1.234,5','Number.toLocaleString');
  eq('I'.toLocaleLowerCase('tr-TR'),'ı','Turkish lower');
  eq('i'.toLocaleUpperCase('tr-TR'),'İ','Turkish upper');
  eq('straße'.toLocaleUpperCase('de-DE'),'STRASSE','case expansion');
  eq(''.toLocaleUpperCase('en-US'),'','empty casing');
  ['h11','h12','h23','h24'].forEach(function(cycle){
    var dt=new Intl.DateTimeFormat('en-US',{timeZone:'UTC',hour:'numeric',hourCycle:cycle});
    eq(dt.resolvedOptions().hourCycle,cycle,cycle+' resolved'); joined(dt,0,cycle);
  });
  ['en-US','en-GB','fr-FR','ja-JP'].forEach(function(locale){
    [true,false].forEach(function(hour12){
      var dt=new Intl.DateTimeFormat(locale,{timeZone:'UTC',hour:'numeric',hour12:hour12});
      var cycle=hour12 ? (locale==='ja-JP'?'h11':'h12') : 'h23';
      eq(dt.resolvedOptions().hourCycle,cycle,locale+' hour12='+hour12+' cycle');
      var parts=joined(dt,0,locale+' midnight hour12='+hour12);
      eq(Number(parts.find(function(p){return p.type==='hour'}).value),cycle==='h12'?12:0,locale+' midnight hour');
    });
  });
  var style=new Intl.DateTimeFormat('en-US',{timeZone:'UTC',dateStyle:'full',timeStyle:'short',hour12:false});
  joined(style,0,'styles'); eq(style.resolvedOptions().hour12,false,'style hour12');
  refuses(function(){new Intl.DateTimeFormat('th-TH')},'default Buddhist refused');
  refuses(function(){new Intl.DateTimeFormat('en-US-u-ca-buddhist')},'calendar extension refused');
  refuses(function(){new Intl.DateTimeFormat('en-US',{calendar:'buddhist'})},'calendar option refused');
  eq(new Intl.DateTimeFormat('th-TH',{calendar:'gregory',timeZone:'UTC'}).resolvedOptions().calendar,'gregory','explicit Gregorian override');
  eq(new Intl.DateTimeFormat('en-US-u-ca-buddhist',{calendar:'gregory',timeZone:'UTC'}).resolvedOptions().calendar,'gregory','extension override');
  refuses(function(){new Intl.DateTimeFormat('en-US',{formatMatcher:'basic'})},'basic format matcher refused');
  refuses(function(){new Intl.DateTimeFormat('en-US',{year:'bad'})},'invalid year refused');
  refuses(function(){new Intl.DateTimeFormat('en-US',{dateStyle:'full',year:'numeric'})},'style/component conflict');
  refuses(function(){new Intl.DateTimeFormat('en-US',{numberingSystem:'foobar'})},'unknown date numbering refused');
  refuses(function(){new Intl.DateTimeFormat('en-US',{timeZone:'Unknown/Zone'})},'timezone refused');
  refuses(function(){new Intl.DateTimeFormat('en-US').format(NaN)},'invalid date refused');
  refuses(function(){new Intl.NumberFormat('en-US',null)},'null NumberFormat options');
  refuses(function(){new Intl.DateTimeFormat('en-US',null)},'null DateTimeFormat options');
  var half=new Intl.NumberFormat('en-US',{maximumFractionDigits:0});
  eq(half.format(2.5),'3','positive half expand'); eq(half.format(-2.5),'-3','negative half expand');
  eq(half.format(-0),'-0','negative zero');
  var precise=new Intl.NumberFormat('en-US',{minimumIntegerDigits:3,maximumSignificantDigits:2,minimumFractionDigits:20,maximumFractionDigits:0,useGrouping:false});
  eq(precise.format(1.234),'001.2','significant precedence + integer padding');
  eq(precise.resolvedOptions().maximumSignificantDigits,2,'significant resolved');
  ok(!('maximumFractionDigits' in precise.resolvedOptions()),'inactive precision omitted');
  ['USD','JPY','KWD'].forEach(function(currency,i){
    var nf=new Intl.NumberFormat('en-US',{style:'currency',currency:currency});
    eq(nf.resolvedOptions().maximumFractionDigits,[2,0,3][i],currency+' default minor digits');
    joined(nf,1234.56,currency);
  });
  var esAuto=new Intl.NumberFormat('es-ES'), esTrue=new Intl.NumberFormat('es-ES',{useGrouping:true});
  eq(esAuto.format(1000),'1000','Spanish omitted grouping'); eq(esTrue.format(1000),'1.000','Spanish explicit grouping');
  var big=new Intl.NumberFormat('en-US',{style:'percent',useGrouping:false});
  ok(big.format(1e300).length>300,'large finite percent is not clipped'); joined(big,1e300,'large percent');
  eq(new Intl.NumberFormat('en-US',{numberingSystem:'arab'}).resolvedOptions().numberingSystem,'arab','effective numbering');
  refuses(function(){new Intl.NumberFormat('en-US',{numberingSystem:'foobar'})},'unknown number system');
  refuses(function(){new Intl.NumberFormat('en-US',{maximumFractionDigits:21})},'digit bounds');
  refuses(function(){new Intl.NumberFormat('en-US',{minimumFractionDigits:3,maximumFractionDigits:1})},'digit order');
  [{style:'unit',unit:'meter'},{notation:'compact'},{currencySign:'accounting'},{currencyDisplay:'name'},
   {signDisplay:'always'},{useGrouping:'min2'},{roundingMode:'halfEven'},{roundingPriority:'morePrecision'},
   {roundingIncrement:2},{trailingZeroDisplay:'stripIfInteger'}].forEach(function(opts){
    refuses(function(){new Intl.NumberFormat('en-US',opts)},'named unsupported '+JSON.stringify(opts));
  });
  for(var i=0;i<100;i++) {
    refuses(function(){new Intl.DateTimeFormat('th-TH')},'failed date construction cleanup');
    refuses(function(){new Intl.NumberFormat('en-US',{currency:'?'})},'failed number construction cleanup');
  }
  var calendarCases=[{"label":"1582-10-04T00:00:00Z","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-12220243200000,"parts":[{"type":"month","value":"10"},{"type":"literal","value":"/"},{"type":"day","value":"04"},{"type":"literal","value":"/"},{"type":"year","value":"1582"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]},{"label":"1582-10-15T00:00:00Z","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-12219292800000,"parts":[{"type":"month","value":"10"},{"type":"literal","value":"/"},{"type":"day","value":"15"},{"type":"literal","value":"/"},{"type":"year","value":"1582"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]},{"label":"0001-01-01T00:00:00Z","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-62135596800000,"parts":[{"type":"month","value":"01"},{"type":"literal","value":"/"},{"type":"day","value":"01"},{"type":"literal","value":"/"},{"type":"year","value":"1"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]},{"label":"0000-01-01T00:00:00Z","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-62167219200000,"parts":[{"type":"month","value":"01"},{"type":"literal","value":"/"},{"type":"day","value":"01"},{"type":"literal","value":"/"},{"type":"year","value":"1"},{"type":"literal","value":" "},{"type":"era","value":"BC"}]},{"label":"-000001-01-01T00:00:00Z","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-62198755200000,"parts":[{"type":"month","value":"01"},{"type":"literal","value":"/"},{"type":"day","value":"01"},{"type":"literal","value":"/"},{"type":"year","value":"2"},{"type":"literal","value":" "},{"type":"era","value":"BC"}]},{"label":"TimeClip -8640000000000000 UTC","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-8640000000000000,"parts":[{"type":"month","value":"04"},{"type":"literal","value":"/"},{"type":"day","value":"20"},{"type":"literal","value":"/"},{"type":"year","value":"271822"},{"type":"literal","value":" "},{"type":"era","value":"BC"}]},{"label":"TimeClip -8640000000000000 America/New_York","locale":"en-US","options":{"timeZone":"America/New_York","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":-8640000000000000,"parts":[{"type":"month","value":"04"},{"type":"literal","value":"/"},{"type":"day","value":"19"},{"type":"literal","value":"/"},{"type":"year","value":"271822"},{"type":"literal","value":" "},{"type":"era","value":"BC"}]},{"label":"TimeClip 8640000000000000 UTC","locale":"en-US","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":8640000000000000,"parts":[{"type":"month","value":"09"},{"type":"literal","value":"/"},{"type":"day","value":"13"},{"type":"literal","value":"/"},{"type":"year","value":"275760"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]},{"label":"TimeClip 8640000000000000 America/New_York","locale":"en-US","options":{"timeZone":"America/New_York","year":"numeric","month":"2-digit","day":"2-digit","era":"short"},"ms":8640000000000000,"parts":[{"type":"month","value":"09"},{"type":"literal","value":"/"},{"type":"day","value":"12"},{"type":"literal","value":"/"},{"type":"year","value":"275760"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]},{"label":"New York DST","locale":"en-US","options":{"timeZone":"America/New_York","year":"numeric","month":"2-digit","day":"2-digit","hour":"2-digit","minute":"2-digit","hourCycle":"h23","timeZoneName":"short"},"ms":1710055800000,"parts":[{"type":"month","value":"03"},{"type":"literal","value":"/"},{"type":"day","value":"10"},{"type":"literal","value":"/"},{"type":"year","value":"2024"},{"type":"literal","value":", "},{"type":"hour","value":"03"},{"type":"literal","value":":"},{"type":"minute","value":"30"},{"type":"literal","value":" "},{"type":"timeZoneName","value":"EDT"}]},{"label":"Arabic numbering","locale":"en-US-u-nu-arab","options":{"timeZone":"UTC","year":"numeric","month":"2-digit","day":"2-digit","era":"short","numberingSystem":"arab"},"ms":0,"parts":[{"type":"month","value":"٠١"},{"type":"literal","value":"/"},{"type":"day","value":"٠١"},{"type":"literal","value":"/"},{"type":"year","value":"١٩٧٠"},{"type":"literal","value":" "},{"type":"era","value":"AD"}]}];
  calendarCases.forEach(function(c){
    var dt=new Intl.DateTimeFormat(c.locale,c.options);
    var parts=joined(dt,c.ms,c.label);
    eq(JSON.stringify(parts),JSON.stringify(c.parts),c.label+" actual V8 parts");
  });
  if (failures.length) throw new Error(JSON.stringify({assertions:assertions,failures:failures,observations:observations}));
  return JSON.stringify({assertions:assertions,observations:observations});
})();
