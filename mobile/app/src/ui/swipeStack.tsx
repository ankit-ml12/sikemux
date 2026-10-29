import { Animated, Dimensions, Easing } from 'react-native';
import {
  useCardAnimation,
  type StackCardInterpolatedStyle,
  type StackCardInterpolationProps,
  type StackNavigationOptions,
} from 'expo-router/js-stack';

/** Screens sit side by side and move together, so a screen going back pulls the one under it in from the left. */
function sideBySide({ current, next, layouts }: StackCardInterpolationProps): StackCardInterpolatedStyle {
  const width = layouts.screen.width;
  const arriving = current.progress.interpolate({ inputRange: [0, 1], outputRange: [width, 0], extrapolate: 'clamp' });
  const covered = next?.progress.interpolate({ inputRange: [0, 1], outputRange: [0, -width], extrapolate: 'clamp' });
  return { cardStyle: { transform: [{ translateX: covered ? Animated.add(arriving, covered) : arriving }] } };
}

const slide: NonNullable<StackNavigationOptions['transitionSpec']>['open'] = {
  animation: 'timing',
  config: { duration: 200, easing: Easing.out(Easing.cubic) },
};

/* Android's own stack takes a screen out of the window as it leaves, which blanks its pictures, and has no drag back,
   so there the screens are drawn by JavaScript instead, and a swipe right anywhere on one drags it back. */
export function swipeOptions(ground: string): StackNavigationOptions {
  return {
    headerShown: false,
    freezeOnBlur: true,
    cardStyle: { backgroundColor: ground },
    cardOverlayEnabled: false,
    cardShadowEnabled: false,
    cardStyleInterpolator: sideBySide,
    transitionSpec: { open: slide, close: slide },
    gestureEnabled: true,
    gestureDirection: 'horizontal',
    gestureResponseDistance: Dimensions.get('screen').width,
  };
}

/** For a layer that stays put while its screen slides, fading out as the screen leaves. */
export function useStillFade() {
  const { current, layouts } = useCardAnimation();
  return {
    opacity: current.progress,
    transform: [
      {
        translateX: current.progress.interpolate({
          inputRange: [0, 1],
          outputRange: [-layouts.screen.width, 0],
          extrapolate: 'clamp',
        }),
      },
    ],
  };
}
