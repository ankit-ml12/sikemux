/* A browser can't mask one view with another. A fade mask is dropped and the content shown; a text mask is shown as its own text, since there the content is only the fill that colours it. */
import { isValidElement, type ReactNode } from 'react';
import { Text, View, type StyleProp, type ViewStyle } from 'react-native';

export default function MaskedView({ style, children, maskElement }: { style?: StyleProp<ViewStyle>; children?: ReactNode; maskElement?: ReactNode }) {
  const textMask = isValidElement(maskElement) && maskElement.type === Text;
  return <View style={style}>{textMask ? maskElement : children}</View>;
}
