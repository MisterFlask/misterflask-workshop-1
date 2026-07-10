import {Composition} from 'remotion';
import {HellrailVideo, TOTAL_FRAMES} from './HellrailVideo';

export const Root: React.FC = () => {
  return (
    <Composition
      id="Hellrail"
      component={HellrailVideo}
      durationInFrames={TOTAL_FRAMES}
      fps={30}
      width={1280}
      height={720}
    />
  );
};
