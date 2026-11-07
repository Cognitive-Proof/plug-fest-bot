// Copy for each verdict the backend can return (see backend classify policy).
export const VERDICTS={
 'ai':{label:'AI-generated',icon:'✦',speech:'Beep boop. Robot-made!',headline:'Tastes like a robot made it.'},
 'includes-ai':{label:'Includes AI',icon:'◐',speech:'Ooh, a pinch of AI!',headline:'A little AI in this one.'},
 'no-ai':{label:'No AI',icon:'✓',speech:'Mmm, home-made!',headline:'Home-made. Delicious.'},
 'unknown':{label:'AI use unknown',icon:'?',speech:'Hmm… mystery snack.',headline:'Can’t tell what’s in this one.'},
};

export const SAMPLES=[
 {file:'a01_text_to_image.jpg',label:'AI'},
 {file:'i01_generative_fill.jpg',label:'Includes AI'},
 {file:'n01_camera_capture.jpg',label:'No AI'},
 {file:'u01_no_manifest.jpg',label:'Unknown'},
];
